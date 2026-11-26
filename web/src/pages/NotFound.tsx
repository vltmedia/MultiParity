import { Link } from 'react-router-dom';

export default function NotFound({ reason }: { reason?: string }) {
  return (
    <div className="boot">
      <div className="panel">
        <h2>{reason ? 'Not for this account' : 'Nothing here'}</h2>
        <p>{reason || 'This address does not match a page. It may have been moved, or the link was cut short.'}</p>
        <Link className="btn pri" to="/">Go to your projects</Link>
      </div>
    </div>
  );
}
