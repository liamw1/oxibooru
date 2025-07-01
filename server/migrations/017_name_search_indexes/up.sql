-- Create extra indexes on CITEXT columns to speed up pattern filters
-- This needs to be done because CITEXT indexes don't work with patterns
CREATE INDEX "pattern_idx_user_name" ON "user" USING BTREE (lower("name") text_pattern_ops);
CREATE INDEX "pattern_idx_tag_name_name" ON "tag_name" USING BTREE (lower("name") text_pattern_ops);
CREATE INDEX "pattern_idx_pool_name_name" ON "pool_name" USING BTREE (lower("name") text_pattern_ops);

-- This prevents a seq_scan when searching posts with tag filter
CREATE INDEX "idx_post_tag_tag_id_post_id" ON "post_tag" USING BTREE ("tag_id", "post_id");