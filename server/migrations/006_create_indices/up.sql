-- This extension speeds up signature index comparison dramatically
CREATE EXTENSION intarray;
CREATE INDEX idx_post_signature_words ON "post_signature" USING GIN ("words" gin__int_ops);

CREATE INDEX idx_comment_post_id ON "comment" USING BTREE ("post_id");
CREATE INDEX idx_comment_user_id ON "comment" USING BTREE ("user_id");
CREATE INDEX idx_comment_score_user_id ON "comment_score" USING BTREE ("user_id");
CREATE INDEX idx_post_user_id ON "post" USING BTREE ("user_id");
CREATE INDEX idx_post_favorite_user_id ON "post_favorite" USING BTREE ("user_id");
CREATE INDEX idx_post_feature_post_id ON "post_feature" USING BTREE ("post_id");
CREATE INDEX idx_post_feature_user_id ON "post_feature" USING BTREE ("user_id");
CREATE INDEX idx_post_note_post_id ON "post_note" USING BTREE ("post_id");
CREATE INDEX idx_post_relation_child_id ON "post_relation" USING BTREE ("child_id");
CREATE INDEX idx_post_score_user_id ON "post_score" USING BTREE ("user_id");
CREATE INDEX idx_post_tag_tag_id ON "post_tag" USING BTREE ("tag_id");
CREATE INDEX idx_tag_category_id ON "tag" USING BTREE ("category_id");
CREATE INDEX idx_tag_implication_child_id ON "tag_implication" USING BTREE ("child_id");
CREATE INDEX idx_tag_name_order ON "tag_name" USING BTREE ("order");
CREATE INDEX idx_tag_suggestion_child_id ON "tag_suggestion" USING BTREE ("child_id");
