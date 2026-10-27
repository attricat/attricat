-- The interface language of the user's latest message, as a BCP 47 tag. The
-- agent replies in it unless asked otherwise.
ALTER TABLE conversations
    ADD COLUMN language TEXT CHECK (language <> '');
