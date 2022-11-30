use sqlx::{Error, Pool, Postgres};

pub(crate) async fn insert_share(
    db_pool: &Pool<Postgres>,
    layer_uuid: String,
    share: String,
) -> Result<bool, Error> {
    let result = sqlx::query!(
        r#"
            INSERT INTO shares (layer_uuid, share)
            VALUES ($1, $2)
        "#,
        layer_uuid,
        share,
    )
    .execute(db_pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub(crate) async fn count_shares(
    db_pool: &Pool<Postgres>,
    layer_uuid: String,
) -> Result<Option<i64>, Error> {
    let result = sqlx::query!(
        r#"
            SELECT COUNT(share) FROM shares
            WHERE layer_uuid = $1
        "#,
        layer_uuid,
    )
    .fetch_one(db_pool)
    .await?;
    Ok(result.count)
}

pub(crate) async fn select_shares(
    db_pool: &Pool<Postgres>,
    layer_uuid: String,
) -> Result<Vec<String>, Error> {
    let result = sqlx::query!(
        r#"
            SELECT share FROM shares
            WHERE layer_uuid = $1
        "#,
        layer_uuid,
    )
    .fetch_all(db_pool)
    .await?;
    Ok(result.into_iter().map(|r| r.share).collect())
}
