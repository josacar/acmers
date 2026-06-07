# Provider API Reference

Each DNS provider in `acmers` implements the `DnsProvider` trait with two methods:
`add_txt` (create a DNS TXT record) and `remove_txt` (delete it).

## Environment Variables

All credentials are passed via environment variables, following acme.sh naming conventions.
Required vars differ per provider. Run `acmers list-providers` to see requirements.

## Implemented Providers (68)

### Amazon Route53 (`aws`)
- **Env:** `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`
- **Auth:** AWS Signature V4 (HMAC-SHA256)
- **API:** XML/HTTPS to `route53.amazonaws.com`
- **Zone:** `/hostedzone/{id}` — found via `ListHostedZones`
- **Record:** `ChangeResourceRecordSets` with `UPSERT`/`DELETE` actions

### Google Cloud DNS (`gcloud`)
- **Env:** `GCLOUD_PROJECT`, `GCLOUD_ACCESS_TOKEN` (or metadata server)
- **Auth:** Bearer token (OAuth2)
- **API:** REST to `dns.googleapis.com`
- **Zone:** `managedZones` → by `dnsName`
- **Record:** `changes` resource with `additions`/`deletions`

### Azure DNS (`azure`)
- **Env:** `AZUREDNS_SUBSCRIPTIONID`, `AZUREDNS_TENANTID`, `AZUREDNS_APPID`, `AZUREDNS_CLIENTSECRET`
- **Auth:** OAuth2 client credentials → Bearer token
- **API:** REST to `management.azure.com`
- **Zone:** `dnsZones` within resource groups
- **Record:** PUT/DELETE TXT records

### Aliyun (`ali`)
- **Env:** `Ali_Key`, `Ali_Secret`
- **Auth:** HMAC-SHA1 Signature V1
- **API:** GET with signed query params to `alidns.aliyuncs.com`
- **Record:** `AddDomainRecord` / `DeleteDomainRecord`

### TencentCloud (`tencent`)
- **Env:** `TENCENT_SecretId`, `TENCENT_SecretKey`
- **Auth:** TC3-HMAC-SHA256
- **API:** POST to `dnspod.tencentcloudapi.com`
- **Record:** `CreateRecord` / `DeleteRecord`

### OVH (`ovh`)
- **Env:** `OVH_AK`, `OVH_AS`, `OVH_CK`, `OVH_END_POINT`
- **Auth:** SHA1 digest signature (`$1$` format)
- **API:** REST to `api.ovh.com`
- **Record:** POST/DELETE to `/domain/zone/{zone}/record`

### DNSPod.cn (`dp`)
- **Env:** `DP_Id`, `DP_Key`
- **Auth:** Login token in form data
- **API:** POST to `dnsapi.cn`
- **Record:** `Record.Create` / `Record.Remove`

### Yandex Cloud (`yc`)
- **Env:** `YC_KeyID`, `YC_Secret`
- **Auth:** IAM token → Bearer
- **API:** REST to `dns.api.cloud.yandex.net`
- **Record:** `upsertRecordSets` with replacements/deletions

### Cloudflare (`cf`)
- **Env:** `CF_Token`, `CF_Zone_ID` (optional), `CF_Account_ID` (optional)
- **Auth:** Bearer token
- **API:** REST to `api.cloudflare.com/client/v4`
- **Zone:** `/zones?name={domain}` → zone `id`

### DigitalOcean (`dgon`)
- **Env:** `DO_API_KEY`
- **Auth:** Bearer token
- **API:** REST to `api.digitalocean.com/v2/domains`
- **Record:** Domain records — `domain_record.id`

### GoDaddy (`gd`)
- **Env:** `GD_Key`, `GD_Secret`
- **Auth:** `sso-key` header
- **API:** REST to `api.godaddy.com/v1/domains`
- **Record:** PUT all records at once

### Gandi LiveDNS (`gandi_livedns`)
- **Env:** `GANDI_LIVEDNS_TOKEN`
- **Auth:** Bearer token
- **API:** REST to `api.gandi.net/v5/livedns`
- **Record:** rrset_name/rrset_values format

### Porkbun (`porkbun`)
- **Env:** `PORKBUN_API_KEY`, `PORKBUN_SECRET_API_KEY`
- **Auth:** API keys in JSON body
- **API:** REST to `api.porkbun.com/api/json/v3`

### Namecheap (`namecheap`)
- **Env:** `NAMECHEAP_API_KEY`, `NAMECHEAP_USERNAME`
- **Auth:** Query params in URL
- **API:** XML/GET to `api.namecheap.com/xml.response`
- **Record:** `domains.dns.getHosts` / `domains.dns.setHosts`

### Name.com (`namecom`)
- **Env:** `Namecom_Username`, `Namecom_Token`
- **Auth:** Basic auth
- **API:** REST to `api.name.com/v4/domains`
- **Record:** `host`/`answer` format

### DNSimple (`dnsimple`)
- **Env:** `DNSimple_OAUTH_TOKEN`
- **Auth:** Bearer token
- **API:** REST to `api.dnsimple.com/v2/{account}/zones`

### Vercel (`vercel`)
- **Env:** `VERCEL_TOKEN`
- **Auth:** Bearer token
- **API:** REST to `api.vercel.com`
- **Record:** `uid` identifier

### Linode v4 (`linode_v4`)
- **Env:** `LINODE_V4_API_KEY`
- **Auth:** Bearer token
- **API:** REST to `api.linode.com/v4/domains`

### Hetzner Cloud DNS (`hetznercloud`)
- **Env:** `HETZNERCLOUD_Token`
- **Auth:** `Auth-API-Token` header
- **API:** REST to `dns.hetzner.com/api/v1`

### IONOS (`ionos`)
- **Env:** `IONOS_PREFIX`, `IONOS_SECRET`
- **Auth:** `X-API-Key` header
- **API:** REST to `api.hosting.ionos.com/dns/v1`

### ClouDNS (`cloudns`)
- **Env:** `CLOUDNS_AUTH_ID`, `CLOUDNS_AUTH_PASSWORD`
- **Auth:** Query params in URL
- **API:** POST to `api.cloudns.net/dns`

### Bunny DNS (`bunny`)
- **Env:** `BUNNY_API_KEY`
- **Auth:** `AccessKey` header
- **API:** REST to `api.bunny.net/dnszone`

### deSEC (`desec`)
- **Env:** `DESEC_Token`
- **Auth:** `Token` header
- **API:** REST to `desec.io/api/v1`

### Njalla (`njalla`)
- **Env:** `NJALLA_Token`
- **Auth:** `Njalla` auth header
- **API:** REST to `njal.la/api/1/`

### Netlify (`netlify`)
- **Env:** `NETLIFY_ACCESS_TOKEN`
- **Auth:** Bearer token
- **API:** REST to `api.netlify.com/api/v1`

### Scaleway (`scaleway`)
- **Env:** `SCALEWAY_API_TOKEN`
- **Auth:** `X-Auth-Token` header
- **API:** REST to `api.scaleway.com/domain/v2beta1`

### Constellix (`constellix`)
- **Env:** `CONSTELLIX_Key`, `CONSTELLIX_Secret`
- **Auth:** `x-cnsdns-apiKey` header
- **API:** REST to `api.dns.constellix.com/v1`

### Vultr (`vultr`)
- **Env:** `VULTR_API_KEY`
- **Auth:** Bearer token
- **API:** REST to `api.vultr.com/v2/domains`

### Exoscale (`exoscale`)
- **Env:** `EXOSCALE_API_KEY`, `EXOSCALE_API_SECRET`
- **Auth:** Basic auth
- **API:** REST to `api.exoscale.com/dns/v1`

### Dynv6 (`dynv6`)
- **Env:** `DYNV6_TOKEN`
- **Auth:** Bearer token
- **API:** REST to `dynv6.com/api/v2`

### Rage4 (`rage4`)
- **Env:** `RAGE4_Key`, `RAGE4_Secret`
- **Auth:** Basic auth
- **API:** GET to `rage4.com/rapi`

### G-Core (`gcore`)
- **Env:** `GCORE_PermanentAPIKey`
- **Auth:** `APIKey` header
- **API:** REST to `api.gcore.com/dns/v2`

### EdgeCenter (`edgecenter`)
- **Env:** `EDGECENTER_API_KEY`
- **Auth:** `APIKey` header
- **API:** REST to `api.edgecenter.ru/dns/v2`

### ACME-DNS (`acmedns`)
- **Env:** `ACMEDNS_URL_BASE`, `ACMEDNS_USERNAME`, `ACMEDNS_PASSWORD`
- **Auth:** Basic auth
- **API:** POST to user-configured URL
- **Note:** Update-in-place, no deletion needed

### DuckDNS (`duckdns`)
- **Env:** `DuckDNS_Token`
- **Auth:** Query param token
- **API:** GET to `duckdns.org/update?domains={}&token={}&txt={}`

### FreeMyIP (`freemyip`)
- **Env:** `FREEMYIP_Token`
- **Auth:** Query param token
- **API:** GET to `freemyip.com/update`

### HE DDNS (`he_ddns`)
- **Env:** `HE_DDNS_Key`
- **Auth:** Query param
- **API:** POST to `dyn.dns.he.net/nic/update`

### DreamHost (`dreamhost`)
- **Env:** `DH_API_KEY`
- **Auth:** Query param key
- **API:** GET to `api.dreamhost.com` with cmd parameters

### DNSExit (`dnsexit`)
- **Env:** `DNSEXIT_API_KEY`
- **Auth:** Query param
- **API:** GET to `api.dnsexit.com/dns`

### DirectAdmin (`da`)
- **Env:** `DA_Api` (URL with user:pass), `DA_Api_Insecure`
- **Auth:** Basic auth from URL
- **API:** POST to `CMD_API_DNS_CONTROL`

### Active24 (`active24`)
- **Env:** `ACTIVE24_Token`
- **Auth:** Bearer token
- **API:** REST to `api.active24.com/v2`

### Simply.com (`simply`)
- **Env:** `SIMPLY_ApiLogin`, `SIMPLY_ApiKey`
- **Auth:** Basic auth
- **API:** REST to `api.simply.com/2`
- **Note:** Formerly UnoEuro

### Mythic Beasts (`mythic_beasts`)
- **Env:** `MYTHIC_BEASTS_Key`, `MYTHIC_BEASTS_Secret`
- **Auth:** Basic auth
- **API:** REST to `api.mythic-beasts.com/dns/v2`

### World4You (`world4you`)
- **Env:** `WORLD4YOU_Username`, `WORLD4YOU_Password`
- **Auth:** AuthToken header
- **API:** REST to `my.world4you.com/api/v1`

### Variomedia (`variomedia`)
- **Env:** `VARIOMEDIA_Email`, `VARIOMEDIA_Token`
- **Auth:** Basic auth
- **API:** REST to `api.variomedia.de`

### Domeneshop (`domeneshop`)
- **Env:** `DOMENESHOP_Key`, `DOMENESHOP_Secret`
- **Auth:** Basic auth
- **API:** REST to `api.domeneshop.no/v0`

### RackCorp (`rackcorp`)
- **Env:** `RACKCORP_UUID`, `RACKCORP_API_KEY`
- **Auth:** Basic auth
- **API:** REST to `api.rackcorp.net/v2`

### Vscale (`vscale`)
- **Env:** `VSCALE_API_KEY`
- **Auth:** `X-Token` header
- **API:** REST to `api.vscale.io/v1`

### ConoHa (`conoha`)
- **Env:** `CONOHA_Username`, `CONOHA_Password`, `CONOHA_TenantId`
- **Auth:** Identity API token → `X-Auth-Token`
- **API:** REST to `dns-service.tyo1.conoha.io`

### EUServ (`euserv`)
- **Env:** `EUSERV_Username`, `EUSERV_Password`
- **Auth:** Basic auth
- **API:** REST to `api.euserv.net/v1`

### PointHQ (`pointhq`)
- **Env:** `POINTHQ_User`, `POINTHQ_Token`
- **Auth:** Basic auth
- **API:** REST to `pointhq.com/api`

### Misaka.io (`misaka`)
- **Env:** `MISAKA_Key`, `MISAKA_Secret`
- **Auth:** Basic auth
- **API:** REST to `api.misaka.io/v1`

### SiteHost (`sitehost`)
- **Env:** `SITEHOST_ApiKey`, `SITEHOST_Secret`
- **Auth:** Basic auth + form POST
- **API:** Form data to `api.sitehost.nz/1.3`

### BookMyName (`bookmyname`)
- **Env:** `BOOKMYNAME_Username`, `BOOKMYNAME_Password`
- **Auth:** Credentials in JSON body
- **API:** POST to `api.bookmyname.com`

### Websupport (`websupport`)
- **Env:** `WEBSUPPORT_Key`, `WEBSUPPORT_Secret`
- **Auth:** Basic auth
- **API:** REST to `rest.websupport.sk/v1`

### Infomaniak (`infomaniak`)
- **Env:** `INFOMANIAK_ACCESS_TOKEN`
- **Auth:** Bearer token
- **API:** REST to `api.infomaniak.com/1`

### MyDevil (`mydevil`)
- **Env:** `MYDEVIL_Username`, `MYDEVIL_Password`
- **Auth:** Basic auth
- **API:** GET/POST to `api.mydevil.net`

### Mijn.host (`mijnhost`)
- **Env:** `MIJNHOST_API_KEY`
- **Auth:** Bearer token
- **API:** REST to `mijn.host/api/v2`

### OpenProvider REST (`openprovider_rest`)
- **Env:** `OPENPROVIDER_REST_Username`, `OPENPROVIDER_REST_Password`
- **Auth:** Login token → Bearer
- **API:** REST to `api.openprovider.eu/v1beta`

### Alwaysdata (`ad`)
- **Env:** `AD_API_KEY`
- **Auth:** API key as Basic username (empty password)
- **API:** REST to `api.alwaysdata.com/v1`

### Restena (`restena`)
- **Env:** `RESTENA_Username`, `RESTENA_Password`
- **Auth:** Basic auth
- **API:** REST to `rest.dns.restena.net/v1`

### Timeweb Cloud (`timeweb`)
- **Env:** `TIMEWEB_Token`
- **Auth:** Bearer token
- **API:** REST to `api.timeweb.cloud/api/v1`

### EasyDNS (`easydns`)
- **Env:** `EASYDNS_Username`, `EASYDNS_Password`, `EASYDNS_APIKey`
- **Auth:** Basic + X-API-Key
- **API:** REST/form to `rest.easydns.net`

### DurableDNS (`durabledns`)
- **Env:** `DURABLEDNS_User`, `DURABLEDNS_Key`
- **Auth:** Query params
- **API:** GET to `api.durabledns.com/dns`

### Internet.bs (`internetbs`)
- **Env:** `INTERNETBS_API_KEY`, `INTERNETBS_API_PASSWORD`
- **Auth:** Query params
- **API:** GET to `api.internet.bs/Domain/DnsRecord`

### Nodion (`nodion`)
- **Env:** `NODION_API_KEY`
- **Auth:** `X-Auth-Token` header
- **API:** REST to `api.nodion.com/v1`

### Technitium (`technitium`)
- **Env:** `TECHNITIUM_Server`, `TECHNITIUM_Token`
- **Auth:** Query param token
- **API:** POST to `{server}/api/zones/records`
