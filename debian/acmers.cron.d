# /etc/cron.d/acmers: renewal fallback for hosts without systemd.
#
# On systemd hosts acmers.timer takes precedence and this entry is skipped.
SHELL=/bin/sh
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

0 */12 * * * root test -x /usr/bin/acmers -a \! -d /run/systemd/system && perl -e 'sleep int(rand(43200))' && /usr/bin/acmers renew --all --env-file /etc/acmers/env
