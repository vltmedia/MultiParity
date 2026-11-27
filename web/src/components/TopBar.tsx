import { Link, NavLink } from 'react-router-dom';
import { useSession } from '../lib/session';
import { Menu, Who } from './ui';

export default function TopBar() {
  const { me, canAdmin, signOut } = useSession();
  if (!me) return null;
  return (
    <header className="topbar">
      <NavLink to="/" className="logo">MultiParity</NavLink>
      <nav aria-label="Main">
        <NavLink to="/" end>Projects</NavLink>
        {canAdmin && <NavLink to="/admin">Admin</NavLink>}
      </nav>
      <div className="acct">
        <Menu label="Account" align="right" className="" button={<><Who id={me.id} name={me.name} /> {me.name}</>}>
          {(close) => (
            <>
              <div style={{ padding: '8px 10px' }}>
                <strong style={{ display: 'block' }}>{me.name}</strong>
                <span className="fine">{me.email}</span>
              </div>
              <hr />
              {me.orgs.map((o) => (
                <div key={o.org.id} style={{ padding: '4px 10px', display: 'flex', gap: 8 }} className="fine">
                  <b style={{ color: 'var(--ink)', fontWeight: 600 }}>{o.org.name}</b>
                  <span>{o.role[0].toUpperCase() + o.role.slice(1)}</span>
                </div>
              ))}
              <hr />
              <Link role="menuitem" to="/look" onClick={close}>Look</Link>
              <Link role="menuitem" to="/get-app" onClick={close}>Get the desktop app</Link>
              <button role="menuitem" onClick={() => { close(); signOut(); }}>Sign out</button>
            </>
          )}
        </Menu>
      </div>
    </header>
  );
}
