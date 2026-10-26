// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/utils/email.js`: SMTP via lettre, same templates.
//!
//! Without SMTP settings (`EMAIL_HOST`, `EMAIL_USER`, `EMAIL_PASS`) nothing is
//! sent: the message is logged (dev) and recorded in the in-memory
//! [`Outbox`], which is how tests check mail without sending any.
//!
//! Client-facing mail sets `Reply-To` to the chef's business email from
//! settings; chef-facing mail goes to that business email (falling back to
//! the login email).

use std::sync::Mutex;

use chrono::NaiveDate;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::Config;
use crate::db::begin_tenant;
use crate::error::AppError;
use crate::rate_limit::AccountLimits;
use crate::state::AppState;

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("invalid email address: {0}")]
    Address(String),
    #[error("could not build the message: {0}")]
    Build(String),
    #[error("SMTP error: {0}")]
    Smtp(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// A message as handed to the transport (recorded in the [`Outbox`] when
/// SMTP isn't configured).
#[derive(Debug, Clone)]
pub struct OutgoingEmail {
    pub to: String,
    pub reply_to: Option<String>,
    pub subject: String,
    pub html: String,
    pub text: String,
    /// (filename, bytes)
    pub attachments: Vec<(String, Vec<u8>)>,
}

/// Messages "sent" while SMTP is not configured (dev and tests). Bounded so a
/// long-running dev server doesn't grow it forever.
#[derive(Default)]
pub struct Outbox(Mutex<Vec<OutgoingEmail>>);

impl Outbox {
    const MAX: usize = 200;

    fn push(&self, email: OutgoingEmail) {
        let mut v = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if v.len() >= Self::MAX {
            v.remove(0);
        }
        v.push(email);
    }

    /// A copy of everything recorded so far.
    pub fn messages(&self) -> Vec<OutgoingEmail> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// HTML-escape like the Node `esc()` helper.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn mailbox(addr: &str) -> Result<Mailbox, EmailError> {
    addr.parse::<Mailbox>()
        .map_err(|_| EmailError::Address(addr.to_string()))
}

fn transport(config: &Config) -> Result<AsyncSmtpTransport<Tokio1Executor>, EmailError> {
    let builder = if config.email_port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.email_host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.email_host)
    }
    .map_err(|e| EmailError::Smtp(e.to_string()))?;
    Ok(builder
        .port(config.email_port)
        .credentials(Credentials::new(
            config.email_user.clone(),
            config.email_pass.clone(),
        ))
        .build())
}

/// Per-account limit on the routes that email a client (`acct.email`).
pub async fn check_limit(state: &AppState, account_id: i64) -> Result<(), AppError> {
    state
        .account_limits
        .email
        .check(&state.pool, &AccountLimits::key(account_id))
        .await
        .map_err(|e| e.with_code())
}

/// Core send. Falls back to logging + the outbox if SMTP isn't configured.
pub async fn send(state: &AppState, email: OutgoingEmail) -> Result<(), EmailError> {
    let config = &state.config;
    if !config.email_configured() {
        if !config.production {
            tracing::info!(
                to = %email.to,
                subject = %email.subject,
                text = %email.text,
                "[EMAIL — dev preview, not sent]"
            );
        }
        state.outbox.push(email);
        return Ok(());
    }

    let mut builder = Message::builder()
        .from(mailbox(&config.email_from)?)
        .to(mailbox(&email.to)?)
        .subject(email.subject.clone());
    if let Some(r) = email.reply_to.as_deref().filter(|r| !r.is_empty()) {
        builder = builder.reply_to(mailbox(r)?);
    }
    let body = MultiPart::alternative_plain_html(email.text.clone(), email.html.clone());
    let message = if email.attachments.is_empty() {
        builder.multipart(body)
    } else {
        let mut mixed = MultiPart::mixed().multipart(body);
        for (name, bytes) in email.attachments {
            let part: SinglePart = Attachment::new(name).body(
                bytes,
                ContentType::parse("application/pdf").expect("valid content type"),
            );
            mixed = mixed.singlepart(part);
        }
        builder.multipart(mixed)
    }
    .map_err(|e| EmailError::Build(e.to_string()))?;

    // One retry after RETRY_DELAY for transient failures (4xx, timeouts,
    // dropped connections). A permanent rejection (5xx) fails straight away.
    let transport = transport(config)?;
    retry_once(
        RETRY_DELAY,
        |e: &lettre::transport::smtp::Error| !e.is_permanent(),
        || transport.send(message.clone()),
    )
    .await
    .map_err(|e| EmailError::Smtp(e.to_string()))?;
    Ok(())
}

/// Pause before the single retry of a failed SMTP send.
pub const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

/// Runs `attempt`; if it fails with an error `retryable` accepts, waits
/// `delay` and runs it once more, returning that second result.
async fn retry_once<T, E, F, Fut>(
    delay: std::time::Duration,
    retryable: impl Fn(&E) -> bool,
    mut attempt: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    match attempt().await {
        Err(e) if retryable(&e) => {
            tracing::warn!("email send failed; retrying once");
            tokio::time::sleep(delay).await;
            attempt().await
        }
        result => result,
    }
}

/// Verifies SMTP credentials at startup so misconfiguration fails loudly.
pub async fn verify_transporter(config: &Config) {
    if !config.email_configured() {
        return;
    }
    match transport(config) {
        Ok(t) => match t.test_connection().await {
            Ok(true) => tracing::info!("SMTP transporter verified"),
            Ok(false) => tracing::warn!("SMTP transporter verify failed"),
            Err(e) => tracing::warn!(error = %e, "SMTP transporter verify failed"),
        },
        Err(e) => tracing::warn!(error = %e, "SMTP transporter verify failed"),
    }
}

/// The settings fields the templates use.
#[derive(Debug, Default, sqlx::FromRow)]
struct Biz {
    business_name: Option<String>,
    email: Option<String>,
    website: Option<String>,
    /// Business email, else the login email.
    notify_email: Option<String>,
}

async fn biz(state: &AppState, account_id: i64) -> Result<Biz, EmailError> {
    let mut tx = begin_tenant(&state.pool, account_id).await?;
    let row = sqlx::query_as::<_, Biz>(
        "SELECT s.business_name, s.email, s.website,
                COALESCE(NULLIF(s.email, ''), a.email) AS notify_email
         FROM accounts a LEFT JOIN settings s ON s.account_id = a.id WHERE a.id = $1",
    )
    .bind(account_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.unwrap_or_default())
}

fn non_empty(v: &Option<String>) -> Option<&str> {
    v.as_deref().filter(|s| !s.is_empty())
}

fn email_header(biz: &str) -> String {
    let biz = esc(biz);
    format!(
        r#"
    <!-- Header -->
    <div style="background:#5E7F57;padding:28px 32px;">
      <div style="font-family:Georgia,serif;font-size:22px;font-weight:700;color:white;letter-spacing:-0.3px;">{biz}</div>
      <div style="color:#C8DFC4;font-size:13px;margin-top:4px;">Private Chef Services</div>
    </div>"#
    )
}

fn email_footer(biz: &str, website: Option<&str>) -> String {
    let footer: Vec<String> = [Some(biz), Some("Private Chef Services"), website]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .map(esc)
        .collect();
    format!(
        r#"
    <!-- Footer -->
    <div style="background:#F9F7F2;padding:16px 32px;border-top:1px solid #EAE8E3;">
      <p style="color:#9CA3AF;font-size:11px;margin:0;text-align:center;">{}</p>
    </div>"#,
        footer.join(" · ")
    )
}

fn page(max_width: u32, inner: &str) -> String {
    format!(
        r#"
<!DOCTYPE html>
<html>
<body style="font-family:'DM Sans',Helvetica,Arial,sans-serif;background:#FBF8F1;margin:0;padding:32px 16px;">
  <div style="max-width:{max_width}px;margin:0 auto;background:white;border-radius:16px;overflow:hidden;box-shadow:0 2px 12px rgba(0,0,0,0.08);">
{inner}
  </div>
</body>
</html>"#
    )
}

fn questions_line(biz_email: &str) -> String {
    if biz_email.is_empty() {
        return String::new();
    }
    let e = esc(biz_email);
    format!(
        r#"<p style="color:#6B7280;font-size:13px;line-height:1.6;margin:0;">
        Questions? Reply to this email or reach us at
        <a href="mailto:{e}" style="color:#5E7F57;">{e}</a>
      </p>"#
    )
}

/// Sent to the client right after they submit the intake form.
pub async fn send_intake_confirmation(
    state: &AppState,
    account_id: i64,
    name: &str,
    email: &str,
    edit_url: &str,
) -> Result<(), EmailError> {
    if email.is_empty() {
        return Ok(());
    }
    let s = biz(state, account_id).await?;
    let biz_name = non_empty(&s.business_name)
        .unwrap_or("Your Chef")
        .to_string();
    let biz_email = non_empty(&s.email)
        .map(str::to_string)
        .unwrap_or_else(|| state.config.email_user.clone());
    let full = if edit_url.starts_with("http") {
        edit_url.to_string()
    } else {
        format!("{}{edit_url}", state.config.client_url)
    };
    let subject = format!("Your booking request has been received — {biz_name}");
    let inner = format!(
        r#"    {header}

    <!-- Body -->
    <div style="padding:32px;">
      <p style="font-size:16px;color:#28302A;margin:0 0 16px;">Hi {name},</p>

      <p style="color:#28302A;line-height:1.6;margin:0 0 16px;">
        Thanks for reaching out! Your booking inquiry has been received and your chef will
        review your preferences and be in touch within <strong>1–2 business days</strong> to
        confirm availability.
      </p>

      <p style="color:#28302A;line-height:1.6;margin:0 0 24px;">
        In the meantime, you can review or update your preferences using the link below:
      </p>

      <!-- CTA -->
      <div style="text-align:center;margin:0 0 28px;">
        <a href="{full}"
           style="display:inline-block;background:#5E7F57;color:white;text-decoration:none;
                  padding:14px 28px;border-radius:10px;font-weight:600;font-size:15px;">
          View My Booking Request →
        </a>
      </div>

      <div style="background:#EEF3EA;border-radius:10px;padding:16px 20px;margin-bottom:24px;">
        <div style="font-size:13px;font-weight:600;color:#3A5C32;margin-bottom:8px;">What happens next?</div>
        <ol style="margin:0;padding-left:20px;color:#28302A;font-size:13px;line-height:2;">
          <li>Your chef reviews your request &amp; preferences</li>
          <li>You'll receive a confirmation with your first session date</li>
          <li>Your booking deposit secures your spot</li>
          <li>Weekly meal prep sessions begin!</li>
        </ol>
      </div>

      {questions}
    </div>

    {footer}"#,
        header = email_header(&biz_name),
        name = esc(name),
        questions = questions_line(&biz_email),
        footer = email_footer(&biz_name, non_empty(&s.website)),
    );
    let text = format!(
        "Hi {name},\n\nThanks for reaching out to {biz_name}! Your booking inquiry has been received.\n\nView or update your request: {full}\n\nYour chef will be in touch within 1–2 business days.\n\n{biz_name}"
    );
    send(
        state,
        OutgoingEmail {
            to: email.to_string(),
            reply_to: s.email.clone().filter(|e| !e.is_empty()),
            subject,
            html: page(520, &inner),
            text,
            attachments: vec![],
        },
    )
    .await
}

/// Inputs for [`send_meal_plan_pdf`].
pub struct MealPlan<'a> {
    pub account_id: i64,
    pub client_name: &'a str,
    pub client_email: &'a str,
    pub session_date: Option<NaiveDate>,
    /// The meal plan, rendered by the server (never bytes from the request).
    pub pdf: Vec<u8>,
    pub serving_size: Option<i32>,
}

/// Sends the meal plan PDF to the client.
pub async fn send_meal_plan_pdf(state: &AppState, m: MealPlan<'_>) -> Result<(), EmailError> {
    let s = biz(state, m.account_id).await?;
    let biz_name = non_empty(&s.business_name)
        .unwrap_or("Your Chef")
        .to_string();
    let date_str = m
        .session_date
        .map(|d| d.format("%A, %B %-d").to_string())
        .unwrap_or_else(|| "your upcoming session".into());
    let subject = format!("Your meal plan — {date_str}");
    let serving = m.serving_size.filter(|n| *n != 0).unwrap_or(4);
    let inner = format!(
        r#"    {header}

    <!-- Body -->
    <div style="padding:32px;">
      <p style="font-size:16px;color:#28302A;margin:0 0 16px;">Hi {name},</p>

      <p style="color:#28302A;line-height:1.6;margin:0 0 16px;">
        Your meal prep is complete and your containers are packed and ready to go!
        Find your full meal plan with reheating instructions attached to this email.
      </p>

      <div style="background:#EEF3EA;border-radius:10px;padding:18px 20px;margin:0 0 24px;">
        <div style="font-size:13px;font-weight:600;color:#3A5C32;margin-bottom:10px;">📋 Quick reminders</div>
        <ul style="margin:0;padding-left:20px;color:#28302A;font-size:13px;line-height:2.2;">
          <li>Keep all containers refrigerated at or below <strong>41°F</strong></li>
          <li>Meals labeled <strong>FREEZE</strong> can be stored up to 3 months frozen</li>
          <li>Reheat colors on the plan: <span style="color:#489E77">■</span> stovetop · <span style="color:#C56A4A">■</span> oven · <span style="color:#5E8FB8">■</span> serve cold</li>
          <li>Use by date shown on each meal card in your plan</li>
        </ul>
      </div>

      <p style="color:#28302A;line-height:1.6;margin:0 0 8px;">
        Serving size: <strong>{serving} servings</strong> per dish.
      </p>

      <p style="color:#6B7280;font-size:13px;line-height:1.6;margin:0;">
        Enjoy your meals! Reply to this email with any questions.
      </p>
    </div>

    {footer}"#,
        header = email_header(&biz_name),
        name = esc(m.client_name),
        footer = email_footer(&biz_name, non_empty(&s.website)),
    );
    let text = format!(
        "Hi {},\n\nYour meal plan for {date_str} is attached.\n\nServing size: {serving} servings per dish.\n\nEnjoy your meals!\n{biz_name}",
        m.client_name
    );
    let date_part: String = m
        .session_date
        .map(|d| d.to_string())
        .unwrap_or_else(|| "session".into())
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    send(
        state,
        OutgoingEmail {
            to: m.client_email.to_string(),
            reply_to: s.email.clone().filter(|e| !e.is_empty()),
            subject,
            html: page(520, &inner),
            text,
            attachments: vec![(format!("meal-plan-{date_part}.pdf"), m.pdf)],
        },
    )
    .await
}

/// Inputs for [`send_invoice_pdf`].
pub struct InvoiceMail<'a> {
    pub account_id: i64,
    pub client_name: &'a str,
    pub client_email: &'a str,
    pub invoice_number: Option<&'a str>,
    pub invoice_total: Option<rust_decimal::Decimal>,
    pub due_date: Option<NaiveDate>,
    /// The invoice, rendered by the server (never bytes from the request).
    pub pdf: Vec<u8>,
    pub business_name: Option<&'a str>,
    /// The client's invoice link, when they can pay it online.
    pub pay_url: Option<String>,
}

/// Sends an invoice PDF to the client.
pub async fn send_invoice_pdf(state: &AppState, m: InvoiceMail<'_>) -> Result<(), EmailError> {
    let s = biz(state, m.account_id).await?;
    let biz_name = m
        .business_name
        .filter(|b| !b.is_empty())
        .or(non_empty(&s.business_name))
        .unwrap_or("Your Chef")
        .to_string();
    let biz_email = non_empty(&s.email)
        .map(str::to_string)
        .unwrap_or_else(|| state.config.email_user.clone());
    let inv_ref = m
        .invoice_number
        .filter(|n| !n.is_empty())
        .unwrap_or("Invoice")
        .to_string();
    let due = m.due_date.map(|d| d.format("%B %-d, %Y").to_string());
    let total = m.invoice_total.map(|t| format!("${:.2}", t.round_dp(2)));
    let client = if m.client_name.is_empty() {
        "there"
    } else {
        m.client_name
    };
    let subject = format!("Your invoice from {biz_name} — {inv_ref}");

    let total_row = total
        .as_ref()
        .map(|t| {
            format!(
                r#"<tr>
            <td style="font-size:13px;color:#3A5C32;font-weight:600;padding:4px 0;">Amount Due</td>
            <td style="font-size:15px;color:#28302A;font-weight:700;text-align:right;padding:4px 0;">{t}</td>
          </tr>"#
            )
        })
        .unwrap_or_default();
    let due_row = due
        .as_ref()
        .map(|d| {
            format!(
                r#"<tr>
            <td style="font-size:13px;color:#3A5C32;font-weight:600;padding:4px 0;">Due Date</td>
            <td style="font-size:13px;color:#28302A;text-align:right;padding:4px 0;">{d}</td>
          </tr>"#
            )
        })
        .unwrap_or_default();
    let biz_html = esc(&biz_name);
    let inner = format!(
        r#"
    <!-- Header -->
    <div style="background:linear-gradient(135deg,#1B2B4B 0%,#2D6A4F 100%);padding:28px 32px;">
      <div style="font-family:Georgia,serif;font-size:22px;font-weight:700;color:white;">{biz_html}</div>
      <div style="color:rgba(255,255,255,0.65);font-size:13px;margin-top:4px;">Invoice {inv_ref}</div>
    </div>

    <!-- Body -->
    <div style="padding:32px;">
      <p style="font-size:16px;color:#28302A;margin:0 0 16px;">Hi {client_esc},</p>

      <p style="color:#28302A;line-height:1.6;margin:0 0 20px;">
        Please find your invoice attached to this email. A summary is below for your reference.
      </p>

      <!-- Summary box -->
      <div style="background:#EEF3EA;border-radius:10px;padding:18px 20px;margin:0 0 24px;">
        <table style="width:100%;border-collapse:collapse;">
          <tr>
            <td style="font-size:13px;color:#3A5C32;font-weight:600;padding:4px 0;">Invoice</td>
            <td style="font-size:13px;color:#28302A;text-align:right;padding:4px 0;">{inv_ref}</td>
          </tr>
          {total_row}
          {due_row}
        </table>
      </div>

      {pay_button}
      {questions}
    </div>

    {footer}"#,
        client_esc = esc(client),
        pay_button = m
            .pay_url
            .as_deref()
            .map(|url| format!(
                r#"<div style="text-align:center;margin:0 0 24px;">
        <a href="{}"
           style="display:inline-block;background:#5E7F57;color:white;text-decoration:none;
                  padding:14px 28px;border-radius:10px;font-weight:600;font-size:15px;">
          Pay online →
        </a>
      </div>"#,
                esc(url)
            ))
            .unwrap_or_default(),
        questions = questions_line(&biz_email),
        footer = email_footer(&biz_name, non_empty(&s.website)),
    );
    let mut text = format!("Hi {client},\n\nPlease find your invoice {inv_ref} attached.");
    if let Some(t) = &total {
        text.push_str(&format!("\n\nAmount Due: {t}"));
    }
    if let Some(d) = &due {
        text.push_str(&format!("\nDue: {d}"));
    }
    if let Some(url) = &m.pay_url {
        text.push_str(&format!("\n\nPay online: {url}"));
    }
    text.push_str(&format!(
        "\n\nQuestions? Reply to this email.\n\n{biz_name}"
    ));

    let safe: String = inv_ref
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let attachments = vec![(format!("invoice-{safe}.pdf"), m.pdf)];
    send(
        state,
        OutgoingEmail {
            to: m.client_email.to_string(),
            reply_to: s.email.clone().filter(|e| !e.is_empty()),
            subject,
            html: page(520, &inner),
            text,
            attachments,
        },
    )
    .await
}

/// Inputs for the day-before reminders ([`send_client_reminder`],
/// [`send_chef_reminder`]).
pub struct Reminder<'a> {
    pub account_id: i64,
    pub session_id: i64,
    pub client_name: &'a str,
    pub client_email: Option<&'a str>,
    pub client_phone: Option<&'a str>,
    pub address: Option<&'a str>,
    pub parking_notes: Option<&'a str>,
    pub allergens: &'a [String],
    /// "Tuesday, October 20 at 10:00" (or without the time).
    pub when: &'a str,
    pub dishes: &'a [String],
    pub serving_size: Option<i32>,
}

fn dish_list_html(dishes: &[String]) -> String {
    if dishes.is_empty() {
        return r#"<p style="color:#6B7280;font-size:13px;margin:0;">Menu to be confirmed.</p>"#
            .into();
    }
    let items: String = dishes
        .iter()
        .map(|d| format!("<li>{}</li>", esc(d)))
        .collect();
    format!(
        r#"<ul style="margin:0;padding-left:20px;color:#28302A;font-size:14px;line-height:1.9;">{items}</ul>"#
    )
}

fn dish_list_text(dishes: &[String]) -> String {
    if dishes.is_empty() {
        return "Menu to be confirmed.".into();
    }
    dishes.iter().map(|d| format!("  - {d}\n")).collect()
}

/// The day-before reminder to the client: when, and what's on the menu.
pub async fn send_client_reminder(state: &AppState, r: &Reminder<'_>) -> Result<(), EmailError> {
    let Some(to) = r.client_email.filter(|e| !e.is_empty()) else {
        return Ok(());
    };
    let s = biz(state, r.account_id).await?;
    let biz_name = non_empty(&s.business_name)
        .unwrap_or("Your Chef")
        .to_string();
    let biz_email = non_empty(&s.email).unwrap_or_default().to_string();
    let serving = r.serving_size.filter(|n| *n != 0).unwrap_or(4);
    let inner = format!(
        r#"    {header}

    <!-- Body -->
    <div style="padding:32px;">
      <p style="font-size:16px;color:#28302A;margin:0 0 16px;">Hi {name},</p>

      <p style="color:#28302A;line-height:1.6;margin:0 0 20px;">
        A quick reminder: your meal prep session is <strong>{when}</strong>.
      </p>

      <div style="background:#EEF3EA;border-radius:10px;padding:18px 20px;margin:0 0 20px;">
        <div style="font-size:13px;font-weight:600;color:#3A5C32;margin-bottom:10px;">🍽️ On the menu ({serving} servings each)</div>
        {dishes}
      </div>

      <p style="color:#28302A;line-height:1.6;margin:0 0 16px;">
        Need to change anything? Just reply to this email.
      </p>
      {questions}
    </div>

    {footer}"#,
        header = email_header(&biz_name),
        name = esc(r.client_name),
        when = esc(r.when),
        dishes = dish_list_html(r.dishes),
        questions = questions_line(&biz_email),
        footer = email_footer(&biz_name, non_empty(&s.website)),
    );
    let text = format!(
        "Hi {},\n\nA quick reminder: your meal prep session is {}.\n\nOn the menu ({serving} servings each):\n{}\nNeed to change anything? Just reply to this email.\n\n{biz_name}",
        r.client_name,
        r.when,
        dish_list_text(r.dishes),
    );
    send(
        state,
        OutgoingEmail {
            to: to.to_string(),
            reply_to: (!biz_email.is_empty()).then_some(biz_email),
            subject: format!("Reminder: your meal prep session {}", r.when),
            html: page(520, &inner),
            text,
            attachments: vec![],
        },
    )
    .await
}

/// The day-before prep sheet to the chef: where to go, who to call, what to
/// avoid, what to cook.
pub async fn send_chef_reminder(state: &AppState, r: &Reminder<'_>) -> Result<(), EmailError> {
    let s = biz(state, r.account_id).await?;
    let Some(chef_email) = non_empty(&s.notify_email).map(str::to_string) else {
        return Ok(());
    };
    let biz_name = non_empty(&s.business_name)
        .unwrap_or("Your Chef")
        .to_string();
    let url = format!("{}/sessions/{}", state.config.client_url, r.session_id);
    let serving = r.serving_size.filter(|n| *n != 0).unwrap_or(4);
    let row = |label: &str, value: &str| {
        format!(
            r#"<tr>
            <td style="font-size:13px;color:#3A5C32;font-weight:600;padding:5px 12px 5px 0;vertical-align:top;width:90px;">{label}</td>
            <td style="font-size:13px;color:#28302A;padding:5px 0;">{}</td>
          </tr>"#,
            esc(value)
        )
    };
    let mut rows = row("When", r.when);
    for (label, value) in [
        ("Address", r.address),
        ("Parking", r.parking_notes),
        ("Phone", r.client_phone),
    ] {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            rows.push_str(&row(label, v));
        }
    }
    let allergens = if r.allergens.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div style="background:#FDECEC;border-radius:10px;padding:14px 18px;margin:0 0 20px;color:#9B2C2C;font-size:14px;">
        ⚠️ <strong>Allergens:</strong> {}
      </div>"#,
            esc(&r.allergens.join(", "))
        )
    };
    let inner = format!(
        r#"    {header}

    <!-- Body -->
    <div style="padding:32px;">
      <p style="font-size:16px;color:#28302A;margin:0 0 16px;">Coming up: <strong>{client}</strong></p>

      <div style="background:#EEF3EA;border-radius:10px;padding:18px 20px;margin:0 0 20px;">
        <table style="width:100%;border-collapse:collapse;">
          {rows}
        </table>
      </div>

      {allergens}

      <div style="font-size:13px;font-weight:600;color:#3A5C32;margin-bottom:8px;">Menu ({serving} servings each)</div>
      {dishes}

      <div style="text-align:center;margin:28px 0 0;">
        <a href="{url}"
           style="display:inline-block;background:#5E7F57;color:white;text-decoration:none;
                  padding:14px 28px;border-radius:10px;font-weight:600;font-size:15px;">
          Open session →
        </a>
      </div>
    </div>

    {footer}"#,
        header = email_header(&biz_name),
        client = esc(r.client_name),
        dishes = dish_list_html(r.dishes),
        footer = email_footer(&biz_name, non_empty(&s.website)),
    );
    let mut text = format!("Coming up: {} — {}\n", r.client_name, r.when);
    for (label, value) in [
        ("Address", r.address),
        ("Parking", r.parking_notes),
        ("Phone", r.client_phone),
    ] {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            text.push_str(&format!("{label}: {v}\n"));
        }
    }
    if !r.allergens.is_empty() {
        text.push_str(&format!("ALLERGENS: {}\n", r.allergens.join(", ")));
    }
    text.push_str(&format!(
        "\nMenu ({serving} servings each):\n{}\nOpen session: {url}",
        dish_list_text(r.dishes)
    ));
    send(
        state,
        OutgoingEmail {
            to: chef_email,
            reply_to: None,
            subject: format!("Coming up: {} — {}", r.client_name, r.when),
            html: page(520, &inner),
            text,
            attachments: vec![],
        },
    )
    .await
}

/// Notifies the chef of a new intake (to the business email, else login email).
pub async fn send_chef_new_intake(
    state: &AppState,
    account_id: i64,
    name: &str,
    email: Option<&str>,
    phone: Option<&str>,
    admin_url: &str,
) -> Result<(), EmailError> {
    let s = biz(state, account_id).await?;
    let Some(chef_email) = non_empty(&s.notify_email).map(str::to_string) else {
        return Ok(());
    };
    let biz_name = non_empty(&s.business_name)
        .unwrap_or("Your Chef")
        .to_string();
    let full = if admin_url.starts_with("http") {
        admin_url.to_string()
    } else {
        format!("{}{admin_url}", state.config.client_url)
    };
    let email = email.filter(|e| !e.is_empty());
    let phone = phone.filter(|p| !p.is_empty());
    let email_row = email
        .map(|e| {
            let e = esc(e);
            format!(
                r#"<tr>
            <td style="font-size:13px;color:#3A5C32;font-weight:600;padding:5px 0;">Email</td>
            <td style="font-size:13px;padding:5px 0;"><a href="mailto:{e}" style="color:#5E7F57;">{e}</a></td>
          </tr>"#
            )
        })
        .unwrap_or_default();
    let phone_row = phone
        .map(|p| {
            format!(
                r#"<tr>
            <td style="font-size:13px;color:#3A5C32;font-weight:600;padding:5px 0;">Phone</td>
            <td style="font-size:13px;color:#28302A;padding:5px 0;">{}</td>
          </tr>"#,
                esc(p)
            )
        })
        .unwrap_or_default();
    let inner = format!(
        r#"    {header}

    <!-- Body -->
    <div style="padding:32px;">
      <p style="font-size:16px;color:#28302A;margin:0 0 16px;">You have a new booking inquiry!</p>

      <div style="background:#EEF3EA;border-radius:10px;padding:18px 20px;margin:0 0 24px;">
        <table style="width:100%;border-collapse:collapse;">
          <tr>
            <td style="font-size:13px;color:#3A5C32;font-weight:600;padding:5px 0;width:80px;">Name</td>
            <td style="font-size:13px;color:#28302A;padding:5px 0;">{name}</td>
          </tr>
          {email_row}
          {phone_row}
        </table>
      </div>

      <div style="text-align:center;margin:0 0 24px;">
        <a href="{full}"
           style="display:inline-block;background:#5E7F57;color:white;text-decoration:none;
                  padding:14px 28px;border-radius:10px;font-weight:600;font-size:15px;">
          Review Inquiry →
        </a>
      </div>

      <p style="color:#6B7280;font-size:13px;line-height:1.6;margin:0;">
        Log in to your dashboard to view their full preferences, change the status, or convert to a client.
      </p>
    </div>

    {footer}"#,
        header = email_header(&biz_name),
        name = esc(name),
        footer = email_footer(&biz_name, non_empty(&s.website)),
    );
    let mut text = format!("New client inquiry from {name}!\n\n");
    if let Some(e) = email {
        text.push_str(&format!("Email: {e}\n"));
    }
    if let Some(p) = phone {
        text.push_str(&format!("Phone: {p}\n"));
    }
    text.push_str(&format!("\nReview the inquiry: {full}"));
    send(
        state,
        OutgoingEmail {
            to: chef_email,
            reply_to: None,
            subject: format!("New client inquiry from {name}"),
            html: page(520, &inner),
            text,
            attachments: vec![],
        },
    )
    .await
}

/// Sends a magic login link to the chef.
pub async fn send_magic_link(
    state: &AppState,
    account_id: i64,
    email: &str,
    verify_url: &str,
) -> Result<(), EmailError> {
    let s = biz(state, account_id).await?;
    let biz_name = non_empty(&s.business_name)
        .unwrap_or("Platebook")
        .to_string();
    let biz_html = esc(&biz_name);
    let inner = format!(
        r#"    <div style="background:#5E7F57;padding:24px 32px;">
      <div style="font-family:Georgia,serif;font-size:20px;font-weight:700;color:white;">{biz_html}</div>
      <div style="color:#C8DFC4;font-size:13px;margin-top:4px;">Chef Dashboard</div>
    </div>
    <div style="padding:32px;">
      <p style="font-size:16px;color:#28302A;margin:0 0 12px;">Here's your login link:</p>
      <p style="color:#6B7280;font-size:14px;line-height:1.6;margin:0 0 28px;">
        Click the button below to sign in to your dashboard.
        This link expires in <strong>15 minutes</strong> and can only be used once.
      </p>
      <div style="text-align:center;margin:0 0 28px;">
        <a href="{verify_url}"
           style="display:inline-block;background:#5E7F57;color:white;text-decoration:none;
                  padding:14px 32px;border-radius:10px;font-weight:600;font-size:15px;">
          Sign in to Dashboard →
        </a>
      </div>
      <p style="color:#9CA3AF;font-size:12px;margin:0;">
        If you didn't request this, you can safely ignore this email.
        Only you can use this link.
      </p>
    </div>"#
    );
    let text = format!(
        "{biz_name} — Login Link\n\nClick here to sign in:\n{verify_url}\n\nThis link expires in 15 minutes. If you didn't request this, ignore this email."
    );
    send(
        state,
        OutgoingEmail {
            to: email.to_string(),
            reply_to: None,
            subject: format!("Your {biz_name} login link"),
            html: page(480, &inner),
            text,
            attachments: vec![],
        },
    )
    .await
}

/// Sends a password-reset link to an account's login email.
pub async fn send_password_reset(
    state: &AppState,
    account_id: i64,
    email: &str,
    reset_url: &str,
) -> Result<(), EmailError> {
    let s = biz(state, account_id).await?;
    let biz_name = non_empty(&s.business_name)
        .unwrap_or("your kitchen")
        .to_string();
    let inner = format!(
        r#"    <div style="padding:32px;">
      <p style="font-size:16px;color:#28302A;margin:0 0 12px;">Reset the password for {biz}</p>
      <p style="color:#6B7280;font-size:14px;line-height:1.6;margin:0 0 28px;">
        Click the button below to choose a new password.
        This link expires in <strong>30 minutes</strong> and can only be used once.
      </p>
      <div style="text-align:center;margin:0 0 28px;">
        <a href="{reset_url}"
           style="display:inline-block;background:#5E7F57;color:white;text-decoration:none;
                  padding:14px 32px;border-radius:10px;font-weight:600;font-size:15px;">
          Choose a new password →
        </a>
      </div>
      <p style="color:#9CA3AF;font-size:12px;margin:0;">
        If you didn't request this, you can safely ignore this email — your password won't change.
      </p>
    </div>"#,
        biz = esc(&biz_name)
    );
    let text = format!(
        "Reset your password\n\nChoose a new password here:\n{reset_url}\n\nThis link expires in 30 minutes. If you didn't request this, ignore this email."
    );
    send(
        state,
        OutgoingEmail {
            to: email.to_string(),
            reply_to: None,
            subject: "Reset your password".into(),
            html: page(480, &inner),
            text,
            attachments: vec![],
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::time::Duration;

    #[tokio::test]
    async fn retries_a_retryable_failure_once() {
        let calls = Cell::new(0);
        let attempt = || {
            calls.set(calls.get() + 1);
            let n = calls.get();
            async move { if n == 1 { Err("transient") } else { Ok(n) } }
        };
        assert_eq!(retry_once(Duration::ZERO, |_| true, attempt).await, Ok(2));
        assert_eq!(calls.get(), 2);
    }

    #[tokio::test]
    async fn gives_up_after_the_second_failure_and_skips_permanent_ones() {
        let calls = Cell::new(0);
        let always_fail = || {
            calls.set(calls.get() + 1);
            async { Err::<(), _>("down") }
        };
        assert_eq!(
            retry_once(Duration::ZERO, |_| true, always_fail).await,
            Err("down")
        );
        assert_eq!(calls.get(), 2, "one retry, not more");

        calls.set(0);
        let permanent = || {
            calls.set(calls.get() + 1);
            async { Err::<(), _>("550 no such user") }
        };
        assert!(
            retry_once(Duration::ZERO, |_| false, permanent)
                .await
                .is_err()
        );
        assert_eq!(calls.get(), 1, "permanent errors aren't retried");
    }

    #[test]
    fn escapes_html() {
        assert_eq!(
            esc(r#"<a href="x">'&'</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&#x27;&amp;&#x27;&lt;/a&gt;"
        );
    }
}
