CREATE TABLE IF NOT EXISTS machine_images (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    machine_id    UUID        NOT NULL REFERENCES vending_machines(id) ON DELETE CASCADE,
    file_path     TEXT        NOT NULL,
    mime_type     TEXT        NOT NULL DEFAULT 'image/jpeg',
    file_size     INTEGER     NOT NULL,
    sha256        TEXT        NOT NULL,
    analyzed      BOOLEAN     NOT NULL DEFAULT FALSE,
    analyze_error TEXT,
    uploaded_by   TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_machine_images_machine_id
    ON machine_images (machine_id, created_at DESC);
