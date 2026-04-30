CREATE TABLE IF NOT EXISTS machine_votes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    machine_id UUID NOT NULL REFERENCES vending_machines(id) ON DELETE CASCADE,
    voter_fingerprint TEXT NOT NULL,
    ip_prefix TEXT,
    vote_type TEXT NOT NULL CHECK (vote_type IN ('exists', 'removed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_vote_per_fingerprint UNIQUE (machine_id, voter_fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_machine_votes_machine_id ON machine_votes (machine_id);
CREATE INDEX IF NOT EXISTS idx_machine_votes_ip_prefix ON machine_votes (machine_id, ip_prefix, created_at DESC);
