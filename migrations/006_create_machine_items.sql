CREATE TABLE IF NOT EXISTS machine_items (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    image_id    UUID        NOT NULL REFERENCES machine_images(id) ON DELETE CASCADE,
    machine_id  UUID        NOT NULL REFERENCES vending_machines(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    price_yen   INTEGER,
    temperature TEXT        NOT NULL DEFAULT 'unknown'
                            CHECK (temperature IN ('hot', 'cold', 'warm', 'unknown')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_machine_items_image_id
    ON machine_items (image_id);

CREATE INDEX IF NOT EXISTS idx_machine_items_machine_id
    ON machine_items (machine_id, created_at DESC);
