# Mailwatch

Mailwatch is a tool which watches folders with mails of [Dovecot](https://www.dovecot.org/), an IMAP-Server. If something is changed it calls a configured command.

I use this in my local email setup. I have a local dovecot running and synchronize remote IMAP accounts to this local IMAP server using [mbsync](https://isync.sourceforge.io/mbsync.html).
Then I use the mail client [aerc](https://git.sr.ht/~rjarry/aerc) to read and modify my mails on the dovecot server.
Now every time a modify something, like change read flag, or move or delete a mail, Mailwatch get notified about the change and then it calls mbsync for the specific account and folder to realize a realtime synchronization of the changed folders.
