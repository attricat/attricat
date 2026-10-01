#!/bin/sh
# Only Cloudflare may open new connections to published container ports on eth0.
set -eu
CF_V4="173.245.48.0/20 103.21.244.0/22 103.22.200.0/22 103.31.4.0/22 141.101.64.0/18 108.162.192.0/18 190.93.240.0/20 188.114.96.0/20 197.234.240.0/22 198.41.128.0/17 162.158.0.0/15 104.16.0.0/13 104.24.0.0/14 172.64.0.0/13 131.0.72.0/22"
iptables -N DOCKER-USER 2>/dev/null || true
iptables -F DOCKER-USER
iptables -A DOCKER-USER -m conntrack --ctstate RELATED,ESTABLISHED -j RETURN
iptables -A DOCKER-USER ! -i eth0 -j RETURN
for net in $CF_V4; do
  iptables -A DOCKER-USER -i eth0 -s "$net" -p tcp -m conntrack --ctorigdstport 443 --ctdir ORIGINAL -j RETURN
  iptables -A DOCKER-USER -i eth0 -s "$net" -p tcp -m conntrack --ctorigdstport 80 --ctdir ORIGINAL -j RETURN
done
iptables -A DOCKER-USER -i eth0 -j DROP
