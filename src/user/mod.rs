pub mod auth;
mod user_impl;
mod users;
use crate::prelude::*;
use argon2::verify_encoded as verify;

use google_authenticator::GoogleAuthenticator;
use rand::random;
pub fn rand_string(size: usize) -> String {
    (0..)
        .map(|_| random::<char>())
        .filter(|c| c.is_ascii())
        .map(char::from)
        .take(size)
        .collect()
}

impl Users {
    fn is_auth(&self, session: &Session) -> bool {
        let option = self.sess.get(session.id);
        if let Some(auth_key) = option {
            auth_key == session.auth_key
        } else {
            false
        }
    }

    #[throws(Error)]
    async fn login(&self, form: &Login, is_signup: bool) -> String {
        let form_pwd = &form.password.as_bytes();
        let form_totp = &form.totp_token;
        let user = self
            .conn
            .get_user_by_email(&form.email.to_lowercase())
            .await
            .map_err(|_| Error::EmailDoesNotExist(form.email.clone()))?;
        let user_pwd = &user.password;
        let pwd_correct = verify(user_pwd, form_pwd)?;
        let is_first_login = is_signup || false;

        let g_auth = GoogleAuthenticator::new();
        let totp_correct = g_auth.verify_code(&user.totp_secret, form_totp, 1, 0);

        // totp can only be checked after user has set it up
        if (pwd_correct && totp_correct) || (pwd_correct && is_first_login) {
            self.set_auth_key(user.id)?
        } else {
            println!("pwd_correct: {:?}, totp_correct: {:?}, is_first_login: {:?}", pwd_correct, totp_correct, is_first_login);
            throw!(Error::UnauthorizedError)
        }
    }
    #[throws(Error)]
    fn logout(&self, session: &Session) {
        if self.is_auth(session) {
            self.sess.remove(session.id)?;
        }
    }

    #[throws(Error)]
    fn set_auth_key_for(&self, user_id: i32, time: Duration) -> String {
        let key = rand_string(10);
        self.sess.insert_for(user_id, key.clone(), time)?;
        key
    }

    #[throws(Error)]
    fn set_auth_key(&self, user_id: i32) -> String {
        let key = rand_string(15);
        self.sess.insert(user_id, key.clone())?;
        key
    }

    #[throws(Error)]
    async fn signup(&self, form: &Signup) {
        form.validate()?;
        let email = &form.email.to_lowercase();
        let password = &form.password;
        let result = self.create_user(email, password, false).await;
        match result {
            Ok(_) => (),
            #[cfg(feature="sqlx")]
            Err(Error::SqlxError(sqlx::Error::Database(error))) => {
                if error.code() == Some("23000".into()) {
                    throw!(Error::EmailAlreadyExists)
                } else {
                    throw!(Error::SqlxError(sqlx::Error::Database(error)))
                }
            }
            Err(error) => {
                throw!(error)
            }
        }
    }

    #[throws(Error)]
    async fn login_for(&self, form: &Login, time: Duration) -> String {
        let form_pwd = &form.password.as_bytes();
        let user = self.conn.get_user_by_email(&form.email.to_lowercase()).await?;
        let user_pwd = &user.password;
        if verify(user_pwd, form_pwd)? {
            self.set_auth_key_for(user.id, time)?
        } else {
            throw!(Error::UnauthorizedError)
        }
    }
}
