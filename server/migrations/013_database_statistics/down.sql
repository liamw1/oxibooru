DROP FUNCTION update_user_statistics,
              update_tag_suggestion_statistics,
              update_tag_implication_statistics,
              update_tag_statistics,
              update_default_tag_category_statistics,
              update_tag_category_statistics,
              update_post_score_statistics,
              update_post_note_statistics,
              update_post_feature_statistics,
              update_post_favorite_statistics,
              update_post_tag_statistics,
              update_post_relation_statistics,
              update_post_statistics,
              update_pool_post_statistics,
              update_pool_statistics,
              update_default_pool_category_statistics,
              update_pool_category_statistics,
              update_comment_score_statistics,
              update_comment_statistics
              CASCADE;

ALTER TABLE "user" DROP COLUMN "custom_avatar_size";
ALTER TABLE "post" DROP COLUMN "custom_thumbnail_size";
ALTER TABLE "post" DROP COLUMN "generated_thumbnail_size";

DROP TABLE "user_statistics",
           "tag_statistics", 
           "tag_category_statistics",
           "post_statistics",
           "pool_statistics",
           "pool_category_statistics",
           "comment_statistics",
           "database_statistics";