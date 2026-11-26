# Working away from the office

People connect to a MultiParity server by its address, for example `https://mp.yourstudio.com`.
On the office network the plain address works (`http://10.0.0.24:18500`). From anywhere else
the server must be reachable over HTTPS. Pick one of the four ways below.

Whatever you pick, the apps need nothing extra: put the address in the desktop app's sign-in
screen, or `mp login https://mp.yourstudio.com`.

| | People install | Open ports | Certificates | Best for |
|---|---|---|---|---|
| Tailscale | Tailscale | none | Tailscale's | Small teams, nothing on the public internet |
| Cloudflare tunnel | nothing | none | Cloudflare's | Office server, contractors who should install nothing |
| Caddy (built in) | nothing | 80 and 443 | Let's Encrypt, automatic | A machine with a public address |
| Cloud server | nothing | 80 and 443 | Let's Encrypt, automatic | No office, or a fully remote team |

Files travel in parts of a few MB, so the 100 MB upload limit some proxies have (Cloudflare's
free plan) does not matter, whatever the size of the files.

## Tailscale

1. Install Tailscale on the server and sign in.
2. Each person installs Tailscale and signs in to the same network (invite them from the
   Tailscale admin page).
3. Connect to the server's Tailscale name, for example `http://uaiserver:18500`, or turn on
   HTTPS certificates in Tailscale and use `https://uaiserver.<your-network>.ts.net`.

Nothing is exposed to the internet. Traffic goes straight between computers when it can; on
networks that block that, it is relayed and slower (`tailscale status` shows which).

## Cloudflare tunnel

For a server that already runs `cloudflared` (UAIServer does):

1. In the Cloudflare dashboard: Zero Trust > Networks > Tunnels > the server's tunnel > Public
   hostname > Add. Hostname: `mp.yourstudio.com`. Service: `http://localhost:18500`.
2. In the server's `.env`:

       MP_PUBLIC_URL=https://mp.yourstudio.com
       MP_CLIENT_IP_HEADER=CF-Connecting-IP

   and, so the plain port is only reachable from the server itself, `MP_BIND=127.0.0.1`
   (leave it out to keep office access on `http://<server>:18500`).
3. `docker compose up -d`.

Check Cloudflare's terms for large file traffic on your plan before moving a whole studio's
data through it.

## Caddy (built in)

For a machine with a public address and ports 80 and 443 free:

1. Point the domain's DNS (an A record) at the machine.
2. In `.env`:

       MP_DOMAIN=mp.yourstudio.com
       MP_TLS=you@yourstudio.com
       MP_PUBLIC_URL=https://mp.yourstudio.com
       MP_CLIENT_IP_HEADER=X-Forwarded-For
       MP_BIND=127.0.0.1

3. `docker compose --profile https up -d`

Caddy gets a certificate from Let's Encrypt on first start and renews it by itself; the email
gets a warning if renewal ever fails. HTTP is redirected to HTTPS. `MP_TLS=internal` uses
Caddy's own certificate instead, for trying it out: computers will not trust that one until
Caddy's root certificate is installed on them.

## Cloud server

Run the same Compose file on a cloud machine and use Caddy as above. Pair it with Cloudflare R2
or Backblaze B2 for file storage (`MP_STORAGE=s3`): downloads from those cost nothing, while
AWS S3 charges for every byte people sync unless the server runs in the same AWS region.

## Sign-in protection

Always on. After 8 wrong passwords for one account within 15 minutes, that account waits until
15 minutes after the first of them; after 20 wrong passwords from one address, that address
waits the same way. The person is told how long. A lock is recorded in the audit log
(`auth.locked`), and an admin setting a new password ends the wait. A wrong email takes as long
as a wrong password, so nobody can find out which emails have accounts.

Behind a proxy every request seems to come from the proxy, so set `MP_CLIENT_IP_HEADER` as shown
above; otherwise the per-address limit counts everyone together. Only set it when the server is
reached through that proxy, since anyone who can reach the plain port could fake the header
(the per-account limit still applies to them).
