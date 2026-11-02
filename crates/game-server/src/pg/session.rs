//! The session gate of the PostgreSQL backend: claiming and ending the login's sessions.

use super::*;

/// A claimed session: AES key and IV, privilege, created, premium and its end.
type ClaimRow = (Vec<u8>, Vec<u8>, i16, i64, Option<i16>, Option<i64>);

impl SessionGate for PgBackend {
    async fn claim(&self, claim: &Claim) -> Result<Option<Claimed>> {
        let row: Option<ClaimRow> = sqlx::query_as(
            "UPDATE sessions SET game_connected_at = now(), expires_at = now() + make_interval(secs => $5)
              WHERE user_id = $1 AND identity = $2 AND server_id = $3 AND expires_at > now()
                AND game_connected_at IS NULL AND ($4::inet IS NULL OR login_ip = $4::inet)
             RETURNING aes_key, aes_iv,
                       -- the login server's privilege byte: the staff level in bits 4 to 6 of the status, 11 for players
                       (SELECT CASE WHEN (status >> 4) & 7 = 0 THEN 11 ELSE (status >> 4) & 7 END
                          FROM accounts WHERE id = sessions.user_id)::smallint,
                       -- as the login server's notification carries it
                       (extract(epoch FROM created_at) * 1000000)::bigint,
                       -- premium (docs/original/item-mall.md deviation 4)
                       (SELECT premium FROM accounts WHERE id = sessions.user_id),
                       (SELECT extract(epoch FROM premium_until)::bigint FROM accounts WHERE id = sessions.user_id)",
        )
        .bind(i32::try_from(claim.user_id)?)
        .bind(&claim.identity[..])
        .bind(i16::from(claim.server_id))
        .bind(claim.ip.map(|ip| ip.to_string()))
        .bind(SESSION_GAME_SECS)
        .fetch_optional(&self.pool)
        .await?;
        let Some((key, iv, privilege, created, premium, premium_until)) = row else { return Ok(None) };
        let keys = SessionKeys { key: key.as_slice().try_into()?, iv: iv.as_slice().try_into()? };
        let premium = Premium { kind: u8::try_from(premium.unwrap_or(0))?, until: premium_until };
        Ok(Some(Claimed { keys, privilege: u8::try_from(privilege)?, created, premium }))
    }

    async fn claimable(&self, claim: &Claim) -> Result<bool> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM sessions
              WHERE user_id = $1 AND identity = $2 AND server_id = $3 AND expires_at > now()
                AND game_connected_at IS NULL AND ($4::inet IS NULL OR login_ip = $4::inet))",
        )
        .bind(i32::try_from(claim.user_id)?)
        .bind(&claim.identity[..])
        .bind(i16::from(claim.server_id))
        .bind(claim.ip.map(|ip| ip.to_string()))
        .fetch_one(&self.pool)
        .await?)
    }

    async fn release(&self, user_id: u32, identity: [u8; 16]) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND identity = $2")
            .bind(i32::try_from(user_id)?)
            .bind(&identity[..])
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
