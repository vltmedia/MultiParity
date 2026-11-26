import { Navigate, Route, Routes, useLocation } from 'react-router-dom';
import type { ReactNode } from 'react';
import { SessionProvider, useSession } from './lib/session';
import { DaemonProvider } from './lib/daemon';
import { ErrorNote, Loading, UiProvider } from './components/ui';
import { MOCK } from './api/mode';
import SignIn from './pages/auth/SignIn';
import AcceptInvite from './pages/auth/AcceptInvite';
import ProjectsPage from './pages/projects/ProjectsPage';
import ProjectLayout from './pages/project/ProjectLayout';
import Today from './pages/project/Today';
import Files from './pages/project/Files';
import HistoryView from './pages/project/History';
import People from './pages/project/People';
import Drafts from './pages/project/Drafts';
import Branches from './pages/project/Branches';
import ChangeDetail from './pages/project/ChangeDetail';
import AdminLayout from './pages/admin/AdminLayout';
import AdminOverview from './pages/admin/Overview';
import AdminPeople from './pages/admin/People';
import AdminPerson from './pages/admin/Person';
import AdminInvites from './pages/admin/Invites';
import AdminOrgs from './pages/admin/Orgs';
import AdminOrg from './pages/admin/Org';
import AdminProjects from './pages/admin/Projects';
import AdminFixing from './pages/admin/Fixing';
import AdminAudit from './pages/admin/Audit';
import NotFound from './pages/NotFound';
import Look from './pages/Look';
import ErrorBoundary from './components/ErrorBoundary';

function RequireAuth({ children }: { children: ReactNode }) {
  const { me, loading, error, refresh } = useSession();
  const loc = useLocation();
  if (loading) return <div className="boot"><Loading what="Signing you in" /></div>;
  if (!me && error) return <div className="boot"><ErrorNote error={error} doing="reach the server" retry={refresh} /></div>;
  if (!me) return <Navigate to={`/sign-in?next=${encodeURIComponent(loc.pathname + loc.search)}`} replace />;
  return <>{children}</>;
}

function RequireAdmin({ children }: { children: ReactNode }) {
  const { canAdmin } = useSession();
  if (!canAdmin) return <NotFound reason="The admin area is for server admins and organization owners and admins." />;
  return <>{children}</>;
}

export default function App() {
  const loc = useLocation();
  return (
    <UiProvider>
      <SessionProvider>
        <DaemonProvider>
          {MOCK && <div className="mockflag" title="Data on this page is made up. Open with ?mock=0 to leave mock mode.">Mock data</div>}
          <ErrorBoundary resetKey={loc.pathname}>
          <Routes>
            <Route path="/sign-in" element={<SignIn />} />
            <Route path="/invite/:token" element={<AcceptInvite />} />
            <Route path="/" element={<RequireAuth><ProjectsPage /></RequireAuth>} />
            <Route path="/look" element={<RequireAuth><Look /></RequireAuth>} />
            <Route path="/p/:projectId" element={<RequireAuth><ProjectLayout /></RequireAuth>}>
              <Route index element={<Navigate to="today" replace />} />
              <Route path="today" element={<Today />} />
              <Route path="files" element={<Files />} />
              <Route path="history" element={<HistoryView />} />
              <Route path="people" element={<People />} />
              <Route path="drafts" element={<Drafts />} />
              <Route path="branches" element={<Branches />} />
              <Route path="changes/:number" element={<ChangeDetail />} />
            </Route>
            <Route path="/admin" element={<RequireAuth><RequireAdmin><AdminLayout /></RequireAdmin></RequireAuth>}>
              <Route index element={<AdminOverview />} />
              <Route path="people" element={<AdminPeople />} />
              <Route path="people/:userId" element={<AdminPerson />} />
              <Route path="invites" element={<AdminInvites />} />
              <Route path="orgs" element={<AdminOrgs />} />
              <Route path="orgs/:orgId" element={<AdminOrg />} />
              <Route path="projects" element={<AdminProjects />} />
              <Route path="fixing" element={<AdminFixing />} />
              <Route path="audit" element={<AdminAudit />} />
            </Route>
            <Route path="*" element={<RequireAuth><NotFound /></RequireAuth>} />
          </Routes>
          </ErrorBoundary>
        </DaemonProvider>
      </SessionProvider>
    </UiProvider>
  );
}
