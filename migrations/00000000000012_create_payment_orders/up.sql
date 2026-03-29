CREATE TABLE payment_orders (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    beam_payment_link_id VARCHAR NOT NULL,
    package_id VARCHAR(50) NOT NULL,
    credits_amount INT NOT NULL,
    price_thb INT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    beam_status VARCHAR(50),
    redirect_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_payment_orders_user_id ON payment_orders(user_id);
CREATE INDEX idx_payment_orders_beam_payment_link_id ON payment_orders(beam_payment_link_id);
CREATE INDEX idx_payment_orders_status ON payment_orders(status);
