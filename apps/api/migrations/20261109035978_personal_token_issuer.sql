-- The personal API token that issued this one, if any. Revoking a token also
-- revokes the tokens it issued, so delegated access ends with its source.
ALTER TABLE personal_api_tokens
    ADD COLUMN issuer_token_id UUID REFERENCES personal_api_tokens (id) ON DELETE SET NULL;

CREATE INDEX personal_api_tokens_issuer_token_id_idx
    ON personal_api_tokens (issuer_token_id)
    WHERE issuer_token_id IS NOT NULL;
