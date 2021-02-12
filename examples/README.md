# Examples

You can find examples for all configuration options in `configs/`.
Spawn servers, containing [httpbin](https://github.com/postmanlabs/httpbin) and [whoami](https://github.com/traefik/whoami) servers with `docker-compose up`


## IPv6

If you want to test IPv6, you need to enable IPv6 for docker.

Put this in your `/etc/docker/daemon.json`

```
{
  "ipv6": true,
  "fixed-cidr-v6": "2001:db8:1::/64"
}
```

Restart your docker daemon.
Run with `docker-compose -f docker-compose-ipv6.yml up`
And add the IPv6 to the config file!
