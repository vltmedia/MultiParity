# Email

With email set up, the server sends:

- **Invites**: people invited by address get the link by email (the link can still be copied).
- **Forgot password**: the sign-in page shows *Forgot your password?*, which emails a link that
  works once, for an hour. Setting a new password signs out the person's other sessions.
- **Requests to release**: when someone asks for a file you have claimed (at most once an hour
  per file).
- **Failed backups**, to every server admin.

Without it nothing breaks: invite links are copied by hand and admins set new passwords.

## Setting it up

In `.env`, the sender, then either an SMTP server or SendGrid, and restart the server.

    MP_EMAIL_FROM="MultiParity <multiparity@yourstudio.com>"

### SMTP (Google Workspace, Microsoft 365, Fastmail, Postmark, Amazon SES, your own server)

    MP_SMTP_HOST=smtp.yourprovider.com
    MP_SMTP_PORT=587               # 587 with starttls (default), 465 with tls
    MP_SMTP_SECURITY=starttls      # starttls | tls | none
    MP_SMTP_USER=multiparity@yourstudio.com
    MP_SMTP_PASSWORD=...           # often an app password, not the account's own

### SendGrid

    MP_SENDGRID_API_KEY=SG....     # a key with "Mail Send" permission

The `MP_EMAIL_FROM` address must be a verified sender (or on an authenticated domain) in
SendGrid.

When both are set, `MP_EMAIL=smtp` or `MP_EMAIL=sendgrid` picks one; `MP_EMAIL=off` turns email
off without removing the settings.

## Checking it

Admin > Fixing > Email shows how mail is sent and has **Send me a test email**, which reports
the provider's error if it fails. Links in emails use `MP_PUBLIC_URL`, so set it to the address
people actually use (for example `https://mp.yourstudio.com`).

To keep messages out of spam, send from your studio's own domain with SPF and DKIM set up for
the provider (each provider's setup page gives the DNS records).
