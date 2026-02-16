CREATE TABLE credit_balances (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    balance         INTEGER NOT NULL DEFAULT 0,
    total_purchased INTEGER NOT NULL DEFAULT 0,
    total_consumed  INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT credit_balances_balance_non_negative CHECK (balance >= 0)
);

CREATE INDEX idx_credit_balances_user_id ON credit_balances (user_id);

CREATE TABLE credit_transactions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    type            VARCHAR NOT NULL
                    CHECK (type IN ('purchase', 'consumption', 'bonus', 'refund', 'adjustment')),
    amount          INTEGER NOT NULL,
    balance_after   INTEGER NOT NULL,
    reference_id    UUID,
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT credit_transactions_amount_nonzero CHECK (amount <> 0)
);

CREATE INDEX idx_credit_transactions_user_id ON credit_transactions (user_id);
CREATE INDEX idx_credit_transactions_created ON credit_transactions (user_id, created_at);
CREATE INDEX idx_credit_transactions_reference ON credit_transactions (reference_id)
    WHERE reference_id IS NOT NULL;
