-- Legacy Chef App SQLite database (Express + better-sqlite3), recorded from the
-- last Express server before it was removed. Synthetic data only (example.com).
-- The seeded starter-library rows (recipes 1-20, their ingredients, templates 1-4)
-- were later replaced with the generic Platebook library; ids 21-35 are unused.
-- Loaded by test/e2e.mjs; regenerate only if the legacy schema itself changes (it can't now).
PRAGMA foreign_keys = OFF;
BEGIN;
CREATE TABLE accounts (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    email                   TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash           TEXT,
    slug                    TEXT NOT NULL UNIQUE,
    onboarding_completed_at TEXT,
    last_login_at           TEXT,
    created_at              TEXT DEFAULT (datetime('now'))
  );
CREATE TABLE settings (
    account_id             INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    business_name          TEXT DEFAULT '',
    chef_name              TEXT,
    phone                  TEXT,
    email                  TEXT,
    website                TEXT DEFAULT '',
    location               TEXT DEFAULT '',
    default_session_window REAL DEFAULT 3.0,
    default_serving_size   INTEGER DEFAULT 4,
    default_service_fee    REAL DEFAULT 0,
    logo_base64            TEXT,
    invoice_prefix         TEXT DEFAULT 'INV',
    deposit_cents          INTEGER DEFAULT 0,
    cancellation_hours     INTEGER DEFAULT 48,
    payment_methods        TEXT DEFAULT '',
    booking_terms          TEXT DEFAULT '',
    response_time_text     TEXT DEFAULT 'within 24 hours',
    service_types          TEXT DEFAULT '[]',
    updated_at             TEXT DEFAULT (datetime('now'))
  );
CREATE TABLE clients (
      id              INTEGER PRIMARY KEY AUTOINCREMENT,
      name            TEXT NOT NULL,
      phone           TEXT,
      email           TEXT,
      address         TEXT,
      household_type  TEXT,
      adults          INTEGER DEFAULT 2,
      children        INTEGER DEFAULT 0,
      serving_size    INTEGER DEFAULT 4,
      allergens       TEXT DEFAULT '[]',
      dietary_styles  TEXT DEFAULT '[]',
      liked_proteins  TEXT DEFAULT '[]',
      disliked_foods  TEXT,
      spice_tolerance TEXT,
      oven_available  INTEGER DEFAULT 1,
      stovetop_type   TEXT,
      equipment       TEXT DEFAULT '[]',
      client_containers INTEGER DEFAULT 0,
      parking_notes   TEXT,
      preferred_day   TEXT,
      preferred_time  TEXT,
      grocery_handling TEXT,
      chef_notes      TEXT,
      is_active       INTEGER DEFAULT 1,
      is_sample       INTEGER DEFAULT 0,
      created_at      TEXT DEFAULT (datetime('now')),
      updated_at      TEXT DEFAULT (datetime('now'))
    , account_id INTEGER REFERENCES accounts(id));
CREATE TABLE recipes (
      id                      INTEGER PRIMARY KEY AUTOINCREMENT,
      name                    TEXT NOT NULL,
      category                TEXT,
      session_fit             TEXT,
      prep_time_min           INTEGER,
      cook_time_min           INTEGER,
      servings_base           INTEGER DEFAULT 4,
      hold_quality            INTEGER,
      reheat_quality          INTEGER,
      key_overlap_ingredients TEXT DEFAULT '[]',
      chef_notes              TEXT,
      raw_instructions        TEXT,
      import_source           TEXT DEFAULT 'manual',
      is_active               INTEGER DEFAULT 1,
      created_at              TEXT DEFAULT (datetime('now'))
    , account_id INTEGER REFERENCES accounts(id));
CREATE TABLE ingredients (
      id             INTEGER PRIMARY KEY AUTOINCREMENT,
      recipe_id      INTEGER REFERENCES recipes(id) ON DELETE CASCADE,
      name           TEXT NOT NULL,
      qty_4_serving  TEXT,
      qty_6_serving  TEXT,
      store_section  TEXT,
      category       TEXT,
      storage_note   TEXT
    );
CREATE TABLE session_dishes (
      id         INTEGER PRIMARY KEY AUTOINCREMENT,
      session_id INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
      recipe_id  INTEGER REFERENCES recipes(id),
      sort_order INTEGER DEFAULT 0
    );
CREATE TABLE session_templates (
      id          INTEGER PRIMARY KEY AUTOINCREMENT,
      name        TEXT NOT NULL,
      description TEXT,
      recipe_ids  TEXT DEFAULT '[]',
      is_default  INTEGER DEFAULT 0,
      created_at  TEXT DEFAULT (datetime('now'))
    , account_id INTEGER REFERENCES accounts(id));
CREATE TABLE grocery_items (
      id              INTEGER PRIMARY KEY AUTOINCREMENT,
      session_id      INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
      ingredient_name TEXT,
      total_quantity  TEXT,
      store_section   TEXT,
      source_dishes   TEXT DEFAULT '[]',
      is_checked      INTEGER DEFAULT 0
    );
CREATE TABLE recipe_import_log (
      id                    INTEGER PRIMARY KEY AUTOINCREMENT,
      recipe_id             INTEGER REFERENCES recipes(id),
      import_method         TEXT DEFAULT 'text',
      raw_input_length      INTEGER,
      confidence_score      INTEGER,
      fields_edited_after_parse TEXT DEFAULT '[]',
      created_at            TEXT DEFAULT (datetime('now'))
    , account_id INTEGER REFERENCES accounts(id));
CREATE TABLE db_meta (
      key   TEXT PRIMARY KEY,
      value TEXT
    );
CREATE TABLE client_intakes (
      id                   INTEGER PRIMARY KEY AUTOINCREMENT,
      -- Contact
      name                 TEXT NOT NULL,
      email                TEXT,
      phone                TEXT,
      address              TEXT,
      -- Household
      household_type       TEXT,
      adults               INTEGER DEFAULT 2,
      children             INTEGER DEFAULT 0,
      preferred_serving_size INTEGER DEFAULT 4,
      -- Dietary
      allergens            TEXT DEFAULT '[]',
      dietary_styles       TEXT DEFAULT '[]',
      liked_proteins       TEXT DEFAULT '[]',
      disliked_foods       TEXT,
      spice_tolerance      TEXT,
      cuisine_preferences  TEXT,
      -- Scheduling
      preferred_day        TEXT,
      preferred_time       TEXT,
      scheduling_notes     TEXT,
      -- Kitchen / Service
      kitchen_notes        TEXT,
      client_containers    INTEGER DEFAULT 1,
      grocery_handling     TEXT DEFAULT 'Client shops',
      number_of_meals      INTEGER,
      -- Notes
      notes_for_chef       TEXT,
      -- Legal
      terms_accepted       INTEGER DEFAULT 0,
      terms_accepted_at    TEXT,
      -- Internal tracking
      status               TEXT DEFAULT 'submitted',
      -- Possible: submitted | deposit_pending | deposit_paid | reviewed_by_chef | booked | cancelled
      payment_status       TEXT DEFAULT 'not_requested',
      -- Manual: not_requested | deposit_requested | invoice_sent | payment_pending | paid_offline | waived
      -- Legacy Stripe: unpaid | pending | paid | refunded | failed
      payment_notes        TEXT,   -- Internal note: payment method, reference, etc.
      stripe_session_id    TEXT,
      stripe_payment_intent TEXT,
      booking_fee_amount   REAL DEFAULT 0,
      booking_fee_paid_at  TEXT,
      pdf_path             TEXT,
      internal_notes       TEXT,
      source               TEXT DEFAULT 'intake_form',
      -- Conversion tracking
      converted_client_id  INTEGER REFERENCES clients(id),
      created_at           TEXT DEFAULT (datetime('now')),
      updated_at           TEXT DEFAULT (datetime('now'))
    , account_id INTEGER REFERENCES accounts(id));
CREATE TABLE invoices (
      id               INTEGER PRIMARY KEY AUTOINCREMENT,
      client_id        INTEGER REFERENCES clients(id),
      session_id       INTEGER REFERENCES sessions(id),
      invoice_number   TEXT,
      status           TEXT DEFAULT 'draft',
      -- Possible: draft | sent | paid | void
      issue_date       TEXT,
      due_date         TEXT,
      client_name      TEXT,
      client_email     TEXT,
      client_address   TEXT,
      business_name    TEXT DEFAULT '',
      chef_name        TEXT,
      chef_email       TEXT,
      chef_phone       TEXT,
      message          TEXT,
      internal_notes   TEXT,
      subtotal         REAL DEFAULT 0,
      tax_rate         REAL DEFAULT 0,
      tax_amount       REAL DEFAULT 0,
      total            REAL DEFAULT 0,
      paid_at          TEXT,
      created_at       TEXT DEFAULT (datetime('now')),
      updated_at       TEXT DEFAULT (datetime('now'))
    , client_phone TEXT, session_date_label TEXT, recipes_summary TEXT DEFAULT '[]', session_start_time TEXT, session_end_time TEXT, total_hours REAL DEFAULT 0, container_count INTEGER DEFAULT 0, store_trip INTEGER DEFAULT 0, grocery_total REAL DEFAULT 0, service_fee REAL DEFAULT 0, deleted_at TEXT, account_id INTEGER REFERENCES accounts(id));
CREATE TABLE invoice_line_items (
      id           INTEGER PRIMARY KEY AUTOINCREMENT,
      invoice_id   INTEGER REFERENCES invoices(id) ON DELETE CASCADE,
      sort_order   INTEGER DEFAULT 0,
      description  TEXT NOT NULL DEFAULT '',
      quantity     REAL DEFAULT 1,
      unit_price   REAL DEFAULT 0,
      amount       REAL DEFAULT 0
    );
CREATE TABLE payment_events (
      id              INTEGER PRIMARY KEY AUTOINCREMENT,
      intake_id       INTEGER REFERENCES client_intakes(id),
      event_type      TEXT,   -- checkout.session.completed, payment_intent.failed, etc.
      stripe_event_id TEXT UNIQUE,  -- NULL for manual events; UNIQUE prevents Stripe duplicate replays
      amount          REAL,
      currency        TEXT DEFAULT 'usd',
      status          TEXT,
      raw_payload     TEXT,   -- full Stripe JSON for audit
      created_at      TEXT DEFAULT (datetime('now'))
    , account_id INTEGER REFERENCES accounts(id));
CREATE TABLE "sessions" (
            id                   INTEGER PRIMARY KEY AUTOINCREMENT,
            client_id            INTEGER NOT NULL REFERENCES clients(id),
            session_date         TEXT,
            session_time         TEXT,
            serving_size         INTEGER DEFAULT 4,
            session_window_hours REAL DEFAULT 3.0,
            status               TEXT DEFAULT 'Pending',
            service_fee          REAL DEFAULT 0,
            ingredient_cost      REAL DEFAULT 0,
            chef_notes           TEXT,
            template_name        TEXT,
            completed_at         TEXT,
            created_at           TEXT DEFAULT (datetime('now')),
            pdf_emailed_at       TEXT
          , account_id INTEGER REFERENCES accounts(id));
INSERT INTO "accounts" ("id", "email", "password_hash", "slug", "onboarding_completed_at", "last_login_at", "created_at") VALUES (1, 'alpha.kitchen@example.com', '$2a$12$NpbwITZrvbqrnuRTSODn2uuqgxZ/Y7C/oYIUeH5FwLmb0tDMJxWdG', 'alpha-kitchen', '2026-10-16 19:09:37', '2026-10-16 19:09:37', '2026-10-16 19:09:37');
INSERT INTO "accounts" ("id", "email", "password_hash", "slug", "onboarding_completed_at", "last_login_at", "created_at") VALUES (2, 'beta.bistro@example.com', '$2a$12$EWtko3h62hn3IbV.3NIydeaMW.ZsIR.89svmWEVA2xt2M2h97ny.S', 'beta-bistro', '2026-10-16 19:09:37', '2026-10-16 19:09:37', '2026-10-16 19:09:37');
INSERT INTO "settings" ("account_id", "business_name", "chef_name", "phone", "email", "website", "location", "default_session_window", "default_serving_size", "default_service_fee", "logo_base64", "invoice_prefix", "deposit_cents", "cancellation_hours", "payment_methods", "booking_terms", "response_time_text", "service_types", "updated_at") VALUES (1, 'Alpha Kitchen', 'Sam', '555-0100', 'biz.alpha.kitchen@example.com', '', '', 3, 4, 150, NULL, 'MC', 5000, 48, '', '', 'within 24 hours', '["Meal prep","Dinner party"]', '2026-10-16 19:09:37');
INSERT INTO "settings" ("account_id", "business_name", "chef_name", "phone", "email", "website", "location", "default_session_window", "default_serving_size", "default_service_fee", "logo_base64", "invoice_prefix", "deposit_cents", "cancellation_hours", "payment_methods", "booking_terms", "response_time_text", "service_types", "updated_at") VALUES (2, 'Beta Bistro', 'Sam', '555-0100', 'biz.beta.bistro@example.com', '', '', 3, 4, 150, NULL, 'MC', 5000, 48, '', '', 'within 24 hours', '["Meal prep","Dinner party"]', '2026-10-16 19:09:37');
INSERT INTO "clients" ("id", "name", "phone", "email", "address", "household_type", "adults", "children", "serving_size", "allergens", "dietary_styles", "liked_proteins", "disliked_foods", "spice_tolerance", "oven_available", "stovetop_type", "equipment", "client_containers", "parking_notes", "preferred_day", "preferred_time", "grocery_handling", "chef_notes", "is_active", "is_sample", "created_at", "updated_at", "account_id") VALUES (1, 'The Chen Family (sample)', NULL, NULL, NULL, 'Small Family', 2, 2, 6, '["Tree Nuts"]', '["Dairy-Free"]', '["Chicken","Beef"]', NULL, 'Mild', 1, NULL, '[]', 0, NULL, 'Sunday', NULL, 'Chef Shops', 'Two young kids, mild spice only, parking in driveway', 1, 1, '2026-10-16 19:09:37', '2026-10-16 19:09:37', 1);
INSERT INTO "clients" ("id", "name", "phone", "email", "address", "household_type", "adults", "children", "serving_size", "allergens", "dietary_styles", "liked_proteins", "disliked_foods", "spice_tolerance", "oven_available", "stovetop_type", "equipment", "client_containers", "parking_notes", "preferred_day", "preferred_time", "grocery_handling", "chef_notes", "is_active", "is_sample", "created_at", "updated_at", "account_id") VALUES (2, 'Alpha Kitchen Client', NULL, 'jo@example.com', NULL, NULL, 2, 0, 4, '["Peanuts"]', '[]', '[]', NULL, NULL, 1, NULL, '["Dutch oven"]', 1, NULL, NULL, NULL, NULL, NULL, 1, 0, '2026-10-16 19:09:37', '2026-10-16 19:09:37', 1);
INSERT INTO "clients" ("id", "name", "phone", "email", "address", "household_type", "adults", "children", "serving_size", "allergens", "dietary_styles", "liked_proteins", "disliked_foods", "spice_tolerance", "oven_available", "stovetop_type", "equipment", "client_containers", "parking_notes", "preferred_day", "preferred_time", "grocery_handling", "chef_notes", "is_active", "is_sample", "created_at", "updated_at", "account_id") VALUES (3, 'Gone Soon', NULL, NULL, NULL, NULL, 2, 0, 4, '[]', '[]', '[]', NULL, NULL, 1, NULL, '[]', 0, NULL, NULL, NULL, NULL, NULL, 0, 0, '2026-10-16 19:09:37', '2026-10-16 19:09:37', 1);
INSERT INTO "clients" ("id", "name", "phone", "email", "address", "household_type", "adults", "children", "serving_size", "allergens", "dietary_styles", "liked_proteins", "disliked_foods", "spice_tolerance", "oven_available", "stovetop_type", "equipment", "client_containers", "parking_notes", "preferred_day", "preferred_time", "grocery_handling", "chef_notes", "is_active", "is_sample", "created_at", "updated_at", "account_id") VALUES (4, 'Beta Bistro Client', NULL, 'jo@example.com', NULL, NULL, 2, 0, 4, '["Peanuts"]', '[]', '[]', NULL, NULL, 1, NULL, '["Dutch oven"]', 1, NULL, NULL, NULL, NULL, NULL, 1, 0, '2026-10-16 19:09:37', '2026-10-16 19:09:37', 2);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (1, 'Lemon-Herb Roast Chicken Thighs', 'Families', 'Session-Friendly', 15, 40, 4, 4, 4, '["chicken","lemon","garlic","thyme","olive oil"]', 'Bone-in thighs stay juicy on reheat; pack pan juices separately', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (2, 'Turkey and Black Bean Chili', 'Families', 'Long-Cook', 20, 60, 4, 5, 5, '["onion","garlic","cumin","tomato","black beans"]', 'Better on day 2; keep toppings on the side', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (3, 'Beef and Vegetable Stew', 'Families', 'Long-Cook', 25, 120, 4, 5, 5, '["beef","carrot","onion","thyme","stock"]', 'Freezes well; thicken with a little flour slurry if needed', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (4, 'Chicken Tikka-Style Curry', 'Couples', 'Session-Friendly', 20, 35, 4, 5, 5, '["chicken","yogurt","garlic","ginger","tomato"]', 'Adjust heat per client; serve with rice cooked fresh or packed separately', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (5, 'Baked Salmon with Dill Yogurt Sauce', 'Couples', 'Session-Friendly', 10, 15, 4, 3, 2, '["salmon","lemon","dill","yogurt"]', 'Best eaten within 2 days; label to reheat gently or serve cold', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (6, 'Pork Carnitas', 'Families', 'Long-Cook', 15, 180, 4, 5, 4, '["pork","orange","onion","cumin","garlic"]', 'Crisp under the broiler before packing; tortillas and salsa on the side', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (7, 'Shrimp and Vegetable Fried Rice', 'Couples', 'Session-Friendly', 20, 15, 4, 3, 3, '["rice","egg","scallion","soy sauce","ginger"]', 'Use day-old rice; reheat in a skillet rather than the microwave', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (8, 'Turkey Meatballs in Marinara', 'Families', 'Session-Friendly', 25, 30, 4, 5, 5, '["tomato","garlic","parmesan","basil","egg"]', 'Kid-friendly; pack pasta separately so it does not soak up the sauce', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (9, 'Chickpea and Spinach Coconut Curry', 'Plant-Based', 'Session-Friendly', 15, 25, 4, 5, 5, '["chickpeas","coconut milk","garlic","ginger","spinach"]', 'Vegan and gluten-free; flavor deepens overnight', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (10, 'Roasted Vegetable Lasagna', 'Plant-Based', 'Long-Cook', 30, 60, 4, 5, 4, '["zucchini","tomato","ricotta","basil","garlic"]', 'Vegetarian; rest 20 minutes before cutting so portions hold their shape', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (11, 'Black Bean and Sweet Potato Enchiladas', 'Plant-Based', 'Session-Friendly', 25, 30, 4, 4, 4, '["black beans","sweet potato","cumin","onion","cheese"]', 'Vegetarian; hold the sauce back a little so tortillas stay intact', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (12, 'Red Lentil Soup', 'Plant-Based', 'Long-Cook', 15, 40, 4, 5, 5, '["lentils","carrot","onion","cumin","lemon"]', 'Vegan; thickens as it sits, so add stock when reheating', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (13, 'Chicken and Rice Casserole', 'Families', 'Session-Friendly', 15, 50, 4, 4, 4, '["chicken","rice","broccoli","cheese","onion"]', 'One-dish dinner; portion before chilling', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (14, 'Braised Short Ribs', 'Couples', 'Long-Cook', 25, 180, 4, 5, 5, '["beef","red wine","carrot","thyme","stock"]', 'Skim fat after chilling; pairs with mashed potatoes or polenta', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (15, 'Sesame Ginger Chicken Stir-Fry', 'Couples', 'Session-Friendly', 20, 15, 4, 3, 3, '["chicken","ginger","garlic","soy sauce","scallion"]', 'Keep vegetables crisp-tender so they survive reheating', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (16, 'Mediterranean Quinoa Salad', 'Plant-Based', 'Session-Friendly', 20, 15, 4, 4, 3, '["quinoa","cucumber","lemon","parsley","olive oil"]', 'Served cold; dress the day it is eaten for the best texture', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (17, 'Shepherd''s Pie', 'Families', 'Long-Cook', 30, 50, 4, 5, 4, '["lamb","potato","carrot","onion","thyme"]', 'Use ground beef for a cottage pie; bake in portion-size dishes', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (18, 'Herb-Crusted Pork Tenderloin', 'Couples', 'Session-Friendly', 15, 25, 4, 3, 3, '["pork","garlic","rosemary","mustard"]', 'Slice just before packing; do not overcook (145°F)', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (19, 'Chicken Tortilla Soup', 'Families', 'Long-Cook', 20, 45, 4, 5, 5, '["chicken","tomato","onion","cumin","lime"]', 'Pack tortilla strips, avocado and cheese on the side', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (20, 'Mushroom Barley Risotto', 'Plant-Based', 'Long-Cook', 15, 50, 4, 4, 3, '["mushroom","barley","parmesan","thyme","stock"]', 'Vegetarian; barley holds up far better than rice when reheated', NULL, 'seeded', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (36, 'Alpha Kitchen Stew', 'Other', 'Session-Friendly', 10, 50, 4, NULL, NULL, '["onion"]', NULL, NULL, 'manual', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "recipes" ("id", "name", "category", "session_fit", "prep_time_min", "cook_time_min", "servings_base", "hold_quality", "reheat_quality", "key_overlap_ingredients", "chef_notes", "raw_instructions", "import_source", "is_active", "created_at", "account_id") VALUES (37, 'Beta Bistro Stew', 'Other', 'Session-Friendly', 10, 50, 4, NULL, NULL, '["onion"]', NULL, NULL, 'manual', 1, '2026-10-16 19:09:37', 2);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (1, 1, 'bone-in chicken thighs', '2.5lb', '3.75lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (2, 1, 'lemons', '2', '3', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (3, 1, 'garlic cloves', '6', '9', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (4, 1, 'fresh thyme', '6sprigs', '9sprigs', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (5, 1, 'olive oil', '3tbsp', '4.5tbsp', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (6, 1, 'baby potatoes', '1.5lb', '2.25lb', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (7, 2, 'ground turkey', '1.5lb', '2.25lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (8, 2, 'black beans', '2cans', '3cans', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (9, 2, 'crushed tomatoes', '28oz', '42oz', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (10, 2, 'yellow onion', '1', '1.5', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (11, 2, 'garlic cloves', '4', '6', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (12, 2, 'chili powder', '2tbsp', '3tbsp', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (13, 2, 'ground cumin', '2tsp', '3tsp', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (14, 3, 'beef chuck, cubed', '2lb', '3lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (15, 3, 'carrots', '4', '6', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (16, 3, 'celery stalks', '3', '4', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (17, 3, 'yellow onion', '1', '2', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (18, 3, 'beef stock', '4cups', '6cups', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (19, 3, 'tomato paste', '2tbsp', '3tbsp', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (20, 3, 'fresh thyme', '4sprigs', '6sprigs', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (21, 3, 'yukon gold potatoes', '1.5lb', '2.25lb', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (22, 4, 'boneless chicken thighs', '1.5lb', '2.25lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (23, 4, 'plain yogurt', '0.5cup', '0.75cup', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (24, 4, 'crushed tomatoes', '14oz', '21oz', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (25, 4, 'heavy cream', '0.5cup', '0.75cup', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (26, 4, 'garlic cloves', '4', '6', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (27, 4, 'fresh ginger', '1tbsp', '1.5tbsp', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (28, 4, 'garam masala', '2tsp', '3tsp', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (29, 5, 'salmon fillets', '1.5lb', '2.25lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (30, 5, 'lemons', '1', '2', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (31, 5, 'fresh dill', '0.25cup', '0.33cup', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (32, 5, 'plain Greek yogurt', '0.75cup', '1cup', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (33, 5, 'olive oil', '1tbsp', '1.5tbsp', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (34, 5, 'green beans', '1lb', '1.5lb', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (35, 6, 'pork shoulder', '3lb', '4.5lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (36, 6, 'oranges', '1', '2', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (37, 6, 'yellow onion', '1', '1.5', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (38, 6, 'garlic cloves', '5', '8', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (39, 6, 'ground cumin', '2tsp', '3tsp', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (40, 6, 'dried oregano', '1tsp', '1.5tsp', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (41, 6, 'corn tortillas', '12', '18', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (42, 7, 'raw shrimp, peeled', '1lb', '1.5lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (43, 7, 'cooked jasmine rice', '4cups', '6cups', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (44, 7, 'eggs', '3', '4', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (45, 7, 'frozen peas and carrots', '1.5cup', '2.25cup', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (46, 7, 'scallions', '4', '6', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (47, 7, 'soy sauce', '3tbsp', '4.5tbsp', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (48, 7, 'sesame oil', '1tbsp', '1.5tbsp', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (49, 8, 'ground turkey', '1.5lb', '2.25lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (50, 8, 'breadcrumbs', '0.5cup', '0.75cup', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (51, 8, 'eggs', '1', '2', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (52, 8, 'parmesan, grated', '0.5cup', '0.75cup', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (53, 8, 'marinara sauce', '24oz', '36oz', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (54, 8, 'garlic cloves', '3', '4', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (55, 8, 'fresh basil', '0.25cup', '0.33cup', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (56, 8, 'spaghetti', '1lb', '1.5lb', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (57, 9, 'chickpeas', '2cans', '3cans', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (58, 9, 'coconut milk', '14oz', '21oz', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (59, 9, 'baby spinach', '5oz', '8oz', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (60, 9, 'yellow onion', '1', '1.5', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (61, 9, 'garlic cloves', '3', '5', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (62, 9, 'fresh ginger', '1tbsp', '1.5tbsp', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (63, 9, 'curry powder', '1tbsp', '1.5tbsp', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (64, 10, 'lasagna noodles', '12', '18', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (65, 10, 'zucchini', '2', '3', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (66, 10, 'red bell peppers', '2', '3', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (67, 10, 'ricotta', '15oz', '22oz', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (68, 10, 'mozzarella, shredded', '2cup', '3cup', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (69, 10, 'marinara sauce', '24oz', '36oz', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (70, 10, 'fresh basil', '0.25cup', '0.33cup', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (71, 11, 'sweet potatoes', '2', '3', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (72, 11, 'black beans', '1cans', '2cans', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (73, 11, 'flour tortillas', '8', '12', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (74, 11, 'red enchilada sauce', '19oz', '28oz', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (75, 11, 'cheddar, shredded', '1.5cup', '2.25cup', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (76, 11, 'yellow onion', '1', '1.5', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (77, 11, 'ground cumin', '1tsp', '1.5tsp', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (78, 12, 'red lentils', '1.5cup', '2.25cup', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (79, 12, 'vegetable stock', '6cups', '9cups', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (80, 12, 'carrots', '2', '3', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (81, 12, 'yellow onion', '1', '1.5', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (82, 12, 'garlic cloves', '3', '4', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (83, 12, 'ground cumin', '2tsp', '3tsp', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (84, 12, 'lemons', '1', '1', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (85, 13, 'boneless chicken breasts', '1.5lb', '2.25lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (86, 13, 'long-grain rice', '1.5cup', '2.25cup', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (87, 13, 'broccoli florets', '3cup', '4.5cup', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (88, 13, 'chicken stock', '3cups', '4.5cups', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (89, 13, 'cheddar, shredded', '1.5cup', '2.25cup', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (90, 13, 'yellow onion', '1', '1.5', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (91, 14, 'bone-in short ribs', '3lb', '4.5lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (92, 14, 'dry red wine', '1.5cup', '2.25cup', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (93, 14, 'beef stock', '2cups', '3cups', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (94, 14, 'carrots', '3', '4', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (95, 14, 'yellow onion', '1', '2', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (96, 14, 'tomato paste', '2tbsp', '3tbsp', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (97, 14, 'fresh thyme', '4sprigs', '6sprigs', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (98, 15, 'boneless chicken breasts', '1.5lb', '2.25lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (99, 15, 'broccoli florets', '3cup', '4.5cup', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (100, 15, 'red bell peppers', '1', '2', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (101, 15, 'snap peas', '8oz', '12oz', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (102, 15, 'soy sauce', '0.25cup', '0.33cup', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (103, 15, 'fresh ginger', '1tbsp', '1.5tbsp', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (104, 15, 'sesame seeds', '1tbsp', '1.5tbsp', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (105, 16, 'quinoa', '1.5cup', '2.25cup', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (106, 16, 'cucumber', '1', '2', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (107, 16, 'cherry tomatoes', '1pint', '1.5pint', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (108, 16, 'chickpeas', '1cans', '2cans', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (109, 16, 'feta, crumbled', '0.5cup', '0.75cup', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (110, 16, 'fresh parsley', '0.5cup', '0.75cup', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (111, 16, 'lemons', '2', '3', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (112, 16, 'olive oil', '0.25cup', '0.33cup', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (113, 17, 'ground lamb', '1.5lb', '2.25lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (114, 17, 'russet potatoes', '2.5lb', '3.75lb', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (115, 17, 'carrots', '2', '3', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (116, 17, 'frozen peas', '1cup', '1.5cup', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (117, 17, 'yellow onion', '1', '1.5', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (118, 17, 'butter', '4tbsp', '6tbsp', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (119, 17, 'beef stock', '1cup', '1.5cup', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (120, 17, 'fresh thyme', '3sprigs', '5sprigs', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (121, 18, 'pork tenderloin', '1.5lb', '2.25lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (122, 18, 'dijon mustard', '2tbsp', '3tbsp', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (123, 18, 'fresh rosemary', '2sprigs', '3sprigs', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (124, 18, 'garlic cloves', '3', '4', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (125, 18, 'breadcrumbs', '0.33cup', '0.5cup', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (126, 18, 'carrots', '1lb', '1.5lb', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (127, 19, 'boneless chicken breasts', '1.5lb', '2.25lb', 'Meat & Seafood', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (128, 19, 'chicken stock', '6cups', '9cups', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (129, 19, 'fire-roasted tomatoes', '14oz', '21oz', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (130, 19, 'black beans', '1cans', '2cans', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (131, 19, 'yellow onion', '1', '1.5', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (132, 19, 'limes', '2', '3', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (133, 19, 'ground cumin', '2tsp', '3tsp', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (134, 19, 'tortilla chips', '4oz', '6oz', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (135, 20, 'pearl barley', '1.5cup', '2.25cup', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (136, 20, 'cremini mushrooms', '1lb', '1.5lb', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (137, 20, 'vegetable stock', '5cups', '7.5cups', 'Canned & Jarred', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (138, 20, 'shallots', '2', '3', 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (139, 20, 'parmesan, grated', '0.5cup', '0.75cup', 'Dairy & Eggs', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (140, 20, 'fresh thyme', '4sprigs', '6sprigs', 'Herbs & Spices', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (141, 20, 'dry white wine', '0.5cup', '0.75cup', 'Pantry & Dry Goods', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (155, 36, 'onion', '1 1/2 cups', NULL, 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (156, 36, 'stock', '2 cups', NULL, NULL, NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (157, 37, 'onion', '1 1/2 cups', NULL, 'Produce', NULL, NULL);
INSERT INTO "ingredients" ("id", "recipe_id", "name", "qty_4_serving", "qty_6_serving", "store_section", "category", "storage_note") VALUES (158, 37, 'stock', '2 cups', NULL, NULL, NULL, NULL);
INSERT INTO "session_dishes" ("id", "session_id", "recipe_id", "sort_order") VALUES (1, 1, 36, 0);
INSERT INTO "session_dishes" ("id", "session_id", "recipe_id", "sort_order") VALUES (2, 2, 36, 0);
INSERT INTO "session_dishes" ("id", "session_id", "recipe_id", "sort_order") VALUES (3, 3, 37, 0);
INSERT INTO "session_templates" ("id", "name", "description", "recipe_ids", "is_default", "created_at", "account_id") VALUES (1, 'Weeknight Starter Trio', 'A good first menu: three different profiles that share garlic and pantry staples.', '[1,9,8]', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "session_templates" ("id", "name", "description", "recipe_ids", "is_default", "created_at", "account_id") VALUES (2, 'Family Favorites', 'Kid-friendly anchors plus a vegetarian option. Shares onion, cheese and stock.', '[3,13,11]', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "session_templates" ("id", "name", "description", "recipe_ids", "is_default", "created_at", "account_id") VALUES (3, 'Date-Night Couples', 'An elevated menu for adult households. Shares thyme, wine and stock.', '[14,5,20]', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "session_templates" ("id", "name", "description", "recipe_ids", "is_default", "created_at", "account_id") VALUES (4, 'Meatless Week', 'Three meatless dishes. Shares lemon, garlic and olive oil.', '[12,10,16]', 1, '2026-10-16 19:09:37', 1);
INSERT INTO "session_templates" ("id", "name", "description", "recipe_ids", "is_default", "created_at", "account_id") VALUES (5, 'Weeknight', 'd', '[36]', 0, '2026-10-16 19:09:37', 1);
INSERT INTO "grocery_items" ("id", "session_id", "ingredient_name", "total_quantity", "store_section", "source_dishes", "is_checked") VALUES (1, 1, 'onion', '1 1/2 cups', 'Produce', '["Alpha Kitchen Stew: 1 1/2 cups"]', 1);
INSERT INTO "grocery_items" ("id", "session_id", "ingredient_name", "total_quantity", "store_section", "source_dishes", "is_checked") VALUES (2, 1, 'stock', '2 cups', 'Other', '["Alpha Kitchen Stew: 2 cups"]', 0);
INSERT INTO "recipe_import_log" ("id", "recipe_id", "import_method", "raw_input_length", "confidence_score", "fields_edited_after_parse", "created_at", "account_id") VALUES (1, 36, 'text', 420, 87, '["name","servings_base"]', '2026-10-16 19:09:37', 1);
INSERT INTO "db_meta" ("key", "value") VALUES ('multitenant_v1', '2026-10-16 19:09:37');
INSERT INTO "client_intakes" ("id", "name", "email", "phone", "address", "household_type", "adults", "children", "preferred_serving_size", "allergens", "dietary_styles", "liked_proteins", "disliked_foods", "spice_tolerance", "cuisine_preferences", "preferred_day", "preferred_time", "scheduling_notes", "kitchen_notes", "client_containers", "grocery_handling", "number_of_meals", "notes_for_chef", "terms_accepted", "terms_accepted_at", "status", "payment_status", "payment_notes", "stripe_session_id", "stripe_payment_intent", "booking_fee_amount", "booking_fee_paid_at", "pdf_path", "internal_notes", "source", "converted_client_id", "created_at", "updated_at", "account_id") VALUES (1, 'Lead Person', 'lead@example.com', NULL, NULL, 'Couple', 2, 0, 4, '["Soy"]', '[]', '[]', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 1, 'Client shops', 4, NULL, 1, '2026-10-16 19:09:37', 'deposit_paid', 'paid_offline', 'cash', NULL, NULL, 50, '2026-10-16 19:09:37', NULL, NULL, 'intake_form', NULL, '2026-10-16 19:09:37', '2026-10-16 19:09:37', 1);
INSERT INTO "invoices" ("id", "client_id", "session_id", "invoice_number", "status", "issue_date", "due_date", "client_name", "client_email", "client_address", "business_name", "chef_name", "chef_email", "chef_phone", "message", "internal_notes", "subtotal", "tax_rate", "tax_amount", "total", "paid_at", "created_at", "updated_at", "client_phone", "session_date_label", "recipes_summary", "session_start_time", "session_end_time", "total_hours", "container_count", "store_trip", "grocery_total", "service_fee", "deleted_at", "account_id") VALUES (1, 2, NULL, 'MC-2026-0001', 'paid', NULL, NULL, '', '', '', '', '', '', '', '', '', 0, 0, 0, 0, '2026-10-16T19:09:37.640Z', '2026-10-16 19:09:37', '2026-10-16 19:09:37', '', '', '[]', '', '', 0, 0, 0, 0, 0, NULL, 1);
INSERT INTO "invoices" ("id", "client_id", "session_id", "invoice_number", "status", "issue_date", "due_date", "client_name", "client_email", "client_address", "business_name", "chef_name", "chef_email", "chef_phone", "message", "internal_notes", "subtotal", "tax_rate", "tax_amount", "total", "paid_at", "created_at", "updated_at", "client_phone", "session_date_label", "recipes_summary", "session_start_time", "session_end_time", "total_hours", "container_count", "store_trip", "grocery_total", "service_fee", "deleted_at", "account_id") VALUES (2, 2, 1, 'MC-2026-0002', 'draft', '2026-10-16', '2026-10-30', 'Alpha Kitchen Client', 'jo@example.com', '', 'Alpha Kitchen', 'Sam', 'biz.alpha.kitchen@example.com', '555-0100', NULL, NULL, 260.75, 0, 0, 260.75, NULL, '2026-10-16 19:09:37', '2026-10-16 19:09:37', '', 'Tuesday, November 3, 2026', '["Alpha Kitchen Stew"]', '09:30', '12:30', 3, 4, 0, 60.25, 200.5, NULL, 1);
INSERT INTO "invoice_line_items" ("id", "invoice_id", "sort_order", "description", "quantity", "unit_price", "amount") VALUES (3, 2, 0, 'Chef Service', 1, 200.5, 200.5);
INSERT INTO "invoice_line_items" ("id", "invoice_id", "sort_order", "description", "quantity", "unit_price", "amount") VALUES (4, 2, 1, 'Grocery Shopping', 1, 60.25, 60.25);
INSERT INTO "payment_events" ("id", "intake_id", "event_type", "stripe_event_id", "amount", "currency", "status", "raw_payload", "created_at", "account_id") VALUES (1, 1, 'manual.mark_paid_offline', NULL, 50, 'usd', 'paid_offline', NULL, '2026-10-16 19:09:37', 1);
INSERT INTO "payment_events" ("id", "intake_id", "event_type", "stripe_event_id", "amount", "currency", "status", "raw_payload", "created_at", "account_id") VALUES (2, 1, 'checkout.session.completed', 'evt_test_123', 50, 'usd', 'paid', '{"id":"evt_test_123","data":{"object":{"amount_total":5000}}}', '2026-05-06T07:08:09.000Z', 1);
INSERT INTO "sessions" ("id", "client_id", "session_date", "session_time", "serving_size", "session_window_hours", "status", "service_fee", "ingredient_cost", "chef_notes", "template_name", "completed_at", "created_at", "pdf_emailed_at", "account_id") VALUES (1, 2, '2026-11-03', '09:30', 4, 3, 'Pending', 200.5, 60.25, NULL, NULL, NULL, '2026-10-16 19:09:37', '2026-10-16 19:09:37', 1);
INSERT INTO "sessions" ("id", "client_id", "session_date", "session_time", "serving_size", "session_window_hours", "status", "service_fee", "ingredient_cost", "chef_notes", "template_name", "completed_at", "created_at", "pdf_emailed_at", "account_id") VALUES (2, 2, '2026-11-10', '09:30', 4, 3, 'Completed', 200.5, 60.25, NULL, NULL, '2026-10-16 19:09:37', '2026-10-16 19:09:37', NULL, 1);
INSERT INTO "sessions" ("id", "client_id", "session_date", "session_time", "serving_size", "session_window_hours", "status", "service_fee", "ingredient_cost", "chef_notes", "template_name", "completed_at", "created_at", "pdf_emailed_at", "account_id") VALUES (3, 4, '2026-11-03', '09:30', 4, 3, 'Pending', 200.5, 60.25, NULL, NULL, NULL, '2026-10-16 19:09:37', NULL, 2);
-- Inserting explicit ids already filled sqlite_sequence; restore the recorded high-water marks.
DELETE FROM sqlite_sequence;
INSERT INTO sqlite_sequence (name, seq) VALUES ('sessions', 3);
INSERT INTO sqlite_sequence (name, seq) VALUES ('accounts', 2);
INSERT INTO sqlite_sequence (name, seq) VALUES ('recipes', 37);
INSERT INTO sqlite_sequence (name, seq) VALUES ('ingredients', 158);
INSERT INTO sqlite_sequence (name, seq) VALUES ('session_templates', 5);
INSERT INTO sqlite_sequence (name, seq) VALUES ('clients', 4);
INSERT INTO sqlite_sequence (name, seq) VALUES ('session_dishes', 3);
INSERT INTO sqlite_sequence (name, seq) VALUES ('grocery_items', 2);
INSERT INTO sqlite_sequence (name, seq) VALUES ('invoices', 2);
INSERT INTO sqlite_sequence (name, seq) VALUES ('invoice_line_items', 4);
INSERT INTO sqlite_sequence (name, seq) VALUES ('client_intakes', 1);
INSERT INTO sqlite_sequence (name, seq) VALUES ('payment_events', 2);
INSERT INTO sqlite_sequence (name, seq) VALUES ('recipe_import_log', 1);
CREATE INDEX idx_session_dishes_session ON session_dishes(session_id);
CREATE INDEX idx_ingredients_recipe ON ingredients(recipe_id);
CREATE INDEX idx_invoices_session  ON invoices(session_id);
CREATE INDEX idx_invoices_client   ON invoices(client_id);
CREATE INDEX idx_grocery_session   ON grocery_items(session_id);
CREATE INDEX idx_sessions_client ON sessions(client_id);
CREATE INDEX idx_sessions_date   ON sessions(session_date);
CREATE INDEX idx_clients_account ON clients(account_id);
CREATE INDEX idx_recipes_account ON recipes(account_id);
CREATE INDEX idx_sessions_account ON sessions(account_id);
CREATE INDEX idx_session_templates_account ON session_templates(account_id);
CREATE INDEX idx_client_intakes_account ON client_intakes(account_id);
CREATE INDEX idx_invoices_account ON invoices(account_id);
CREATE INDEX idx_recipe_import_log_account ON recipe_import_log(account_id);
CREATE INDEX idx_payment_events_account ON payment_events(account_id);
CREATE UNIQUE INDEX idx_invoices_account_number ON invoices(account_id, invoice_number);
CREATE UNIQUE INDEX idx_templates_account_name ON session_templates(account_id, name);
COMMIT;
