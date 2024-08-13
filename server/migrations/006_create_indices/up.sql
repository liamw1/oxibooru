-- This extension speeds up signature index comparison dramatically
CREATE EXTENSION intarray;
CREATE INDEX idx_post_signature_words ON post_signature USING GIN (words gin__int_ops);