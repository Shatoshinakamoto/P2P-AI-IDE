//! live_accounts -- drives the IDE core's P2P-AI account sign-in (ADR-030 phase A3) against a real account service
//! and a real Go gateway. Run by `scripts/live_accounts_e2e.mjs`, which starts both services and plays the
//! person in a real (headless) browser. Docs: docs/commands/live-accounts-e2e.md.
//!
//! Protocol with the driver (stdout lines, one JSON object each):
//!   `{"open": "<sign-in url>"}`   the driver opens it in Chromium and signs in;
//!   `{"step": "<name>", "ok": bool, "detail": "..."}` one per check; the exit code is the number of failed steps.
//! The driver writes the one-time password and the new password on stdin (two lines) when asked with
//! `{"need": "passwords"}`, so no secret is ever on the command line (CWE-214).
//!
//! Inputs: `--home <issuer> --audience <GATEWAY_ID> --gateway <url> --data <dir>` (all loopback, test only).
//! Security: SECURITY-WARNING: plain http on 127.0.0.1 only (INSECURE — DEV ONLY); the secret store here is in
//! memory (the app uses the OS keychain); tokens are never printed (only their shape).
//! Dependencies: p2p-ide-core only.

use p2p_ide_core::accounts::{AccountLink, Accounts, AcctError};
use p2p_ide_core::client::{Client, Msg, NetError};
use p2p_ide_core::gateways::{Gateway, GwError, Kind, SecretStore};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

#[derive(Default)]
struct Mem(RefCell<HashMap<String, String>>);
impl SecretStore for Mem {
    fn get(&self, id: &str) -> Result<Option<String>, GwError> {
        Ok(self.0.borrow().get(id).cloned())
    }
    fn set(&self, id: &str, s: &str) -> Result<(), GwError> {
        self.0.borrow_mut().insert(id.into(), s.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), GwError> {
        self.0.borrow_mut().remove(id);
        Ok(())
    }
}

fn say(v: serde_json::Value) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{v}");
    let _ = o.flush();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
    let (home, audience, gw_url, data) =
        (arg("--home"), arg("--audience"), arg("--gateway"), PathBuf::from(arg("--data")));
    let mut fails = 0;
    let mut step = |name: &str, ok: bool, detail: String| {
        if !ok {
            fails += 1;
        }
        say(serde_json::json!({"step": name, "ok": ok, "detail": detail}));
    };

    let acc = Accounts::new(&data);
    let sec = Mem::default();
    let open = |url: &str| {
        say(serde_json::json!({"open": url}));
        true
    };
    let view = acc.sign_in(&sec, &home, &open, &AtomicBool::new(false));
    let Ok(view) = view else {
        step("browser sign-in returns to the IDE", false, view.err().map(|e| e.message()).unwrap_or_default());
        std::process::exit(fails);
    };
    step(
        "browser sign-in returns to the IDE; refresh token in the secret store, none in the file",
        view.signed_in && !std::fs::read_to_string(data.join("accounts.json")).unwrap_or_default().contains("p2prt_"),
        format!("email={} pcr={}", view.email, view.password_change_required),
    );
    step("profile email shown", view.email == "ide-e2e@example.org", view.email.clone());

    let link = AccountLink::new(&home, &audience).expect("link");
    // An operator-made account must pick its own password before any gateway token.
    let first = acc.token_for(&sec, &link);
    step(
        "no gateway token before the one-time password is replaced",
        view.password_change_required && first == Err(AcctError::PasswordChange),
        format!("{:?}", first.as_ref().err()),
    );
    say(serde_json::json!({"need": "passwords"}));
    let mut lines = std::io::stdin().lock().lines();
    let otp = lines.next().and_then(Result::ok).unwrap_or_default();
    let newpw = lines.next().and_then(Result::ok).unwrap_or_default();
    let short = acc.change_password(&sec, &view.id, &otp, "short");
    step("too-short new password refused before it is sent", short == Err(AcctError::BadPassword), String::new());
    let ch = acc.change_password(&sec, &view.id, &otp, &newpw);
    step("password changed from the IDE", ch.is_ok(), ch.err().map(|e| e.message()).unwrap_or_default());

    let tok = acc.token_for(&sec, &link);
    step("gateway token issued (JWT shape)", tok.as_ref().is_ok_and(|t| t.matches('.').count() == 2), String::new());
    let tok = tok.unwrap_or_default();
    let again = acc.token_for(&sec, &link).unwrap_or_default();
    step("second call uses the cached token", again == tok, String::new());

    let gw = Gateway {
        id: "0123456789abcdef".into(),
        name: "e2e".into(),
        url: gw_url.clone(),
        kind: Kind::Manual,
        ca_pem: None,
        account: Some(link.clone()),
    };
    let client = Client::new(&gw, Some(tok.clone())).expect("client");
    let models = client.models();
    step(
        "gateway lists models for the account token",
        models.as_ref().is_ok_and(|(m, _)| m.iter().any(|x| x == "m")),
        format!("{:?}", models.as_ref().map(|m| m.0.clone()).map_err(NetError::message)),
    );
    let mut text = String::new();
    let end = client.chat_stream("m", &[Msg { role: "user".into(), content: "hi".into() }], &mut |d| {
        text.push_str(d);
        true
    });
    step(
        "streamed chat answered with the account token",
        end.is_ok() && text.contains("fake model"),
        format!("{text} {:?}", end.map_err(|e| e.message())),
    );

    // A token minted for another gateway id is refused by this gateway (audience check).
    let other = AccountLink::new(&home, "gw-not-this-one").unwrap();
    let wrong = acc.token_for(&sec, &other).unwrap_or_default();
    let c2 = Client::new(&gw, Some(wrong)).expect("client");
    let r = c2.models();
    step(
        "token for another gateway id is refused by this gateway",
        r.as_ref().err() == Some(&NetError::Auth),
        format!("{:?}", r.err()),
    );

    let out = acc.sign_out(&sec, &view.id);
    step(
        "sign out revokes and forgets",
        out.is_ok() && acc.views(&sec).map(|v| v.is_empty()).unwrap_or(false),
        String::new(),
    );
    let after = acc.token_for(&sec, &link);
    step("no token after sign-out", after == Err(AcctError::NotSignedIn), format!("{:?}", after.err()));
    std::process::exit(fails);
}
