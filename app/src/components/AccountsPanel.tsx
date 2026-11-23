/**
 * AccountsPanel.tsx -- "P2P-AI accounts" in the Gateways side bar (universal accounts, ADR-030 phase A3;
 * docs/features/account-sign-in.md).
 *
 * Purpose: sign in at an account service (the "home") in the system browser, see which account is signed in,
 * replace an operator-made one-time password, and sign out. Gateways are then linked to an account in their own
 * card (NetworkView.tsx), so no key has to be pasted for them.
 * Trust boundary: everything goes through ipc.ts to the Rust core, which checks it. The password fields are password
 * inputs; their values go once to the core (which sends them to the home) and are cleared at once. Tokens never
 * reach the webview. Addresses and emails are rendered as React text (CWE-79).
 */
import { useEffect, useState } from "react";
import { LogIn, LogOut, ShieldAlert, UserRound } from "lucide-react";
import {
  accountChangePassword, accountSignIn, accountSignInCancel, accountSignOut, accountsList, type AccountHome,
} from "../ipc.ts";
import { Tip } from "./Tip.tsx";

interface Props {
  /** Called after a sign-in or sign-out, so the gateway cards can refresh. */
  onChanged: (homes: AccountHome[]) => void;
  onNotice: (m: string) => void;
}

const input = "w-full rounded border border-ide-line bg-ide-bg px-2 py-1 text-ide-bright placeholder:text-ide-dim";
const btn = "rounded px-2 py-1 hover:bg-ide-hover";

export function AccountsPanel({ onChanged, onNotice }: Props) {
  const [homes, setHomes] = useState<AccountHome[]>([]);
  const [home, setHome] = useState("");
  const [waiting, setWaiting] = useState(false);
  const [pwFor, setPwFor] = useState<string | null>(null);
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");

  const load = async () => {
    try {
      const h = await accountsList();
      setHomes(h);
      onChanged(h);
    } catch (e) {
      onNotice((e as Error).message);
    }
  };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { void load(); }, []);

  const signIn = async (address: string) => {
    setWaiting(true);
    try {
      const h = await accountSignIn(address);
      onNotice(h.password_change_required ? "Signed in. Choose your own password to use gateways." : `Signed in${h.email ? ` as ${h.email}` : ""}`);
      if (h.password_change_required) setPwFor(h.id);
      setHome("");
    } catch (e) {
      onNotice((e as Error).message);
    } finally {
      setWaiting(false);
      await load();
    }
  };

  const changePassword = async (id: string) => {
    const c = current, n = next;
    // SECURITY: the passwords leave the form at once.
    setCurrent(""); setNext("");
    try {
      await accountChangePassword(id, c, n);
      setPwFor(null);
      onNotice("Password changed. Your other signed-in devices were signed out.");
    } catch (e) {
      onNotice((e as Error).message);
    }
    await load();
  };

  return (
    <section aria-label="P2P-AI accounts" className="rounded border border-ide-line p-2 text-xs" data-testid="accounts-panel">
      <div className="flex items-center gap-2">
        <UserRound size={14} aria-hidden className="text-ide-dim" />
        <span className="font-semibold text-ide-bright">P2P-AI accounts</span>
      </div>
      <ul aria-label="Account services" className="flex flex-col gap-1 pt-1">
        {homes.map((h) => (
          <li key={h.id} className="rounded border border-ide-line p-1">
            <div className="truncate text-ide-bright" title={h.issuer}>{h.email || h.issuer}</div>
            <div className="truncate text-ide-dim" title={h.issuer}>{h.issuer}</div>
            <div className="flex flex-wrap items-center gap-1 pt-1">
              {h.dev ? <span className="flex items-center gap-1 rounded bg-[#5a1d1d] px-1.5 text-white"><ShieldAlert size={12} aria-hidden />INSECURE — DEV ONLY</span> : null}
              {h.signed_in ? <span className="text-[#89d185]">signed in</span> : <span className="text-[#cca700]">signed out</span>}
              {h.password_change_required ? <span className="text-[#cca700]">choose your own password</span> : null}
            </div>
            <div className="flex flex-wrap gap-1 pt-1">
              {h.signed_in ? (
                <>
                  <button type="button" className={btn} onClick={() => setPwFor(pwFor === h.id ? null : h.id)}>Change password</button>
                  <button type="button" className={btn} onClick={() => void accountSignOut(h.id).then(() => onNotice("Signed out"), (e: Error) => onNotice(e.message)).then(load)}><LogOut size={12} className="mr-1 inline" aria-hidden />Sign out</button>
                </>
              ) : (
                <button type="button" className={btn} disabled={waiting} onClick={() => void signIn(h.issuer)}><LogIn size={12} className="mr-1 inline" aria-hidden />Sign in again</button>
              )}
            </div>
            {pwFor === h.id ? (
              <form className="flex flex-col gap-1 pt-1" onSubmit={(e) => { e.preventDefault(); void changePassword(h.id); }}>
                <input aria-label="Current password" type="password" autoComplete="current-password" value={current} onChange={(e) => setCurrent(e.target.value)} className={input} placeholder="current (or one-time) password" />
                <input aria-label="New password" type="password" autoComplete="new-password" value={next} onChange={(e) => setNext(e.target.value)} className={input} placeholder="new password, 15+ characters" />
                <button type="submit" className="self-start rounded bg-ide-accent px-2 text-white">Change</button>
              </form>
            ) : null}
          </li>
        ))}
      </ul>
      {waiting ? (
        <div className="flex items-center gap-2 pt-1" role="status">
          <span className="text-ide-dim">Finish signing in in your browser...</span>
          <button type="button" className={btn} onClick={() => void accountSignInCancel()}>Cancel</button>
        </div>
      ) : (
        <form className="flex gap-1 pt-1" onSubmit={(e) => { e.preventDefault(); void signIn(home); }}>
          <Tip text="The address of the account service (it is also the gateway operator's account home). Sign-in happens in your browser; the IDE never sees your password.">
            <input aria-label="Account service address" value={home} onChange={(e) => setHome(e.target.value)} className={input} placeholder="https://accounts.example.org" />
          </Tip>
          <button type="submit" className="flex shrink-0 items-center gap-1 whitespace-nowrap rounded bg-ide-accent px-2 text-white"><LogIn size={12} aria-hidden />Sign in</button>
        </form>
      )}
    </section>
  );
}
