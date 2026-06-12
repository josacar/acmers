pub mod cf;
pub mod acmedns;
pub mod acmeproxy;
pub mod active24;
pub mod ad;
pub mod ali;
pub mod alviy;
pub mod anikeen;
pub mod anx;
pub mod area7;
pub mod artfiles;
pub mod aruba;
pub mod arvan;
pub mod aurora;
pub mod autodns;
pub mod aws;
pub mod azion;
pub mod azure;
pub mod baidu;
pub mod beget;
pub mod bh;
pub mod bhosted;
pub mod bookmyname;
pub mod bunny;
pub mod clouddns;
pub mod cloudns;
pub mod cn;
pub mod conoha;
pub mod constellix;
pub mod cpanel;
pub mod curanet;
pub mod cyon;
pub mod czechia;
pub mod da;
pub mod ddnss;
pub mod desec;
pub mod df;
pub mod dgon;
pub mod dnsexit;
pub mod dnshome;
pub mod dnsimple;
pub mod dnsservices;
pub mod doapi;
pub mod domeneshop;
pub mod dp;
pub mod dpi;
pub mod dreamhost;
pub mod duckdns;
pub mod durabledns;
pub mod r#dyn;
pub mod dynadot;
pub mod dynu;
pub mod dynv6;
pub mod easydns;
pub mod edgecenter;
pub mod edgedns;
pub mod efficientip;
pub mod eurodns;
pub mod euserv;
pub mod exoscale;
pub mod firestorm;
pub mod fornex;
pub mod freedns;
pub mod freemyip;
pub mod gandi_livedns;
pub mod gcloud;
pub mod gcore;
pub mod gd;
pub mod geoscaling;
pub mod gname;
pub mod googledomains;
pub mod he;
pub mod he_ddns;
pub mod hestiacp;
pub mod hetzner;
pub mod hetznercloud;
pub mod hexonet;
pub mod hosting1984;
pub mod hostingde;
pub mod hostingukraine;
pub mod hostline;
pub mod hosttech;
pub mod hostup;
pub mod huaweicloud;
pub mod infoblox;
pub mod infoblox_uddi;
pub mod infomaniak;
pub mod internetbs;
pub mod inwx;
pub mod ionos;
pub mod ionos_cloud;
pub mod ipprojects;
pub mod ipv64;
pub mod ispconfig;
pub mod ispman;
pub mod jd;
pub mod joker;
pub mod kappernet;
pub mod kas;
pub mod kinghost;
pub mod knot;
pub mod la;
pub mod leaseweb;
pub mod lexicon;
pub mod limacity;
pub mod linode;
pub mod linode_v4;
pub mod loopia;
pub mod lua;
pub mod maradns;
pub mod me;
pub mod mgwm;
pub mod miab;
pub mod mijnhost;
pub mod misaka;
pub mod myapi;
pub mod mydevil;
pub mod mydnsjp;
pub mod myloc;
pub mod mythic_beasts;
pub mod namecheap;
pub mod namecom;
pub mod namesilo;
pub mod nanelo;
pub mod nederhost;
pub mod neodigit;
pub mod netcup;
pub mod netim;
pub mod netlify;
pub mod nic;
pub mod njalla;
pub mod nm;
pub mod nodion;
pub mod nsd;
pub mod nsone;
pub mod nsupdate;
pub mod nw;
pub mod oci;
pub mod omglol;
pub mod one;
pub mod online;
pub mod openprovider;
pub mod openprovider_rest;
pub mod openstack;
pub mod opnsense;
pub mod opusdns;
pub mod ovh;
pub mod pdns;
pub mod pdnsmanager;
pub mod pleskxml;
pub mod pmiab;
pub mod pointhq;
pub mod porkbun;
pub mod poweradmin;
pub mod qc;
pub mod rackcorp;
pub mod rackspace;
pub mod rage4;
pub mod rcode0;
pub mod regru;
pub mod restena;
pub mod samba;
pub mod scaleway;
pub mod schlundtech;
pub mod sdns;
pub mod selectel;
pub mod selfhost;
pub mod shellrent;
pub mod simply;
pub mod sitehost;
pub mod sotoon;
pub mod spaceship;
pub mod subreg;
pub mod synology_dsm;
pub mod technitium;
pub mod tele3;
pub mod tencent;
pub mod timeweb;
pub mod transip;
pub mod udr;
pub mod ultra;
pub mod unoeuro;
pub mod variomedia;
pub mod veesp;
pub mod vercel;
pub mod virakcloud;
pub mod vscale;
pub mod vultr;
pub mod websupport;
pub mod wedos;
pub mod west_cn;
pub mod wexbo;
pub mod world4you;
pub mod wts;
pub mod yandex;
pub mod yandex360;
pub mod yc;
pub mod zilore;
pub mod zone;
pub mod zoneedit;
pub mod zonomi;
pub mod helpers;

use std::collections::HashMap;

use crate::error::Error;

pub type ProviderResult = Result<(), Error>;

pub trait DnsProvider: Send + Sync {
    fn slug() -> &'static str where Self: Sized;
    fn env_vars() -> &'static [&'static str] where Self: Sized;
    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> where Self: Sized;
    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult;
    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult;
}

pub struct ProviderMeta {
    pub slug: &'static str,
    pub name: &'static str,
    pub env_vars: &'static [&'static str],
    pub create: fn(&HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error>,
}

pub static PROVIDERS: &[ProviderMeta] = &[
    ProviderMeta { slug: "cf", name: "Cloudflare", env_vars: &["CF_Token", "CF_Key", "CF_Email", "CF_Zone_ID", "CF_Account_ID"], create: |env| cf::Cloudflare::new(env) },
    ProviderMeta { slug: "acmedns", name: "Acmedns", env_vars: &["ACMEDNS_URL_BASE", "ACMEDNS_USERNAME", "ACMEDNS_PASSWORD", "ACMEDNS_SUBDOMAIN"], create: |env| acmedns::Acmedns::new(env) },
    ProviderMeta { slug: "acmeproxy", name: "Acmeproxy", env_vars: &["ACMEPROXY_ENDPOINT", "ACMEPROXY_USERNAME", "ACMEPROXY_PASSWORD"], create: |env| acmeproxy::Acmeproxy::new(env) },
    ProviderMeta { slug: "active24", name: "Active24", env_vars: &["Active24_ApiKey", "Active24_ApiSecret"], create: |env| active24::Active24::new(env) },
    ProviderMeta { slug: "ad", name: "Alwaysdata", env_vars: &["AD_API_KEY"], create: |env| ad::Alwaysdata::new(env) },
    ProviderMeta { slug: "ali", name: "Aliyun", env_vars: &["Ali_Key", "Ali_Secret"], create: |env| ali::Aliyun::new(env) },
    ProviderMeta { slug: "alviy", name: "Alviy", env_vars: &["Alviy_token"], create: |env| alviy::Alviy::new(env) },
    ProviderMeta { slug: "anikeen", name: "Anikeen", env_vars: &["ANIKEEN_USERNAME", "ANIKEEN_PASSWORD"], create: |env| anikeen::Anikeen::new(env) },
    ProviderMeta { slug: "anx", name: "Anx", env_vars: &["ANX_Token"], create: |env| anx::Anx::new(env) },
    ProviderMeta { slug: "area7", name: "Area7", env_vars: &["AREA7_API_KEY"], create: |env| area7::Area7::new(env) },
    ProviderMeta { slug: "artfiles", name: "Artfiles", env_vars: &["AF_API_USERNAME", "AF_API_PASSWORD"], create: |env| artfiles::Artfiles::new(env) },
    ProviderMeta { slug: "aruba", name: "Aruba", env_vars: &["ARUBA_USERNAME", "ARUBA_PASSWORD"], create: |env| aruba::Aruba::new(env) },
    ProviderMeta { slug: "arvan", name: "Arvan", env_vars: &["ARVAN_API_KEY"], create: |env| arvan::Arvan::new(env) },
    ProviderMeta { slug: "aurora", name: "Aurora", env_vars: &["AURORA_Key", "AURORA_Secret"], create: |env| aurora::Aurora::new(env) },
    ProviderMeta { slug: "autodns", name: "Autodns", env_vars: &["AUTODNS_USER", "AUTODNS_PASSWORD", "AUTODNS_CONTEXT"], create: |env| autodns::Autodns::new(env) },
    ProviderMeta { slug: "aws", name: "Route53", env_vars: &["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"], create: |env| aws::Route53::new(env) },
    ProviderMeta { slug: "azion", name: "Azion", env_vars: &["AZION_Email", "AZION_Password"], create: |env| azion::Azion::new(env) },
    ProviderMeta { slug: "azure", name: "Azure", env_vars: &["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_TENANTID", "AZUREDNS_APPID", "AZUREDNS_CLIENTSECRET"], create: |env| azure::Azure::new(env) },
    ProviderMeta { slug: "baidu", name: "Baidu", env_vars: &["Baidu_AK", "Baidu_SK"], create: |env| baidu::Baidu::new(env) },
    ProviderMeta { slug: "beget", name: "Beget", env_vars: &["Beget_Username", "Beget_Password"], create: |env| beget::Beget::new(env) },
    ProviderMeta { slug: "bh", name: "BestHosting", env_vars: &["BH_API_USER", "BH_API_KEY"], create: |env| bh::BestHosting::new(env) },
    ProviderMeta { slug: "bhosted", name: "Bhosted", env_vars: &["BHOSTED_Username", "BHOSTED_Password"], create: |env| bhosted::Bhosted::new(env) },
    ProviderMeta { slug: "bookmyname", name: "Bookmyname", env_vars: &["BOOKMYNAME_Username", "BOOKMYNAME_Password"], create: |env| bookmyname::Bookmyname::new(env) },
    ProviderMeta { slug: "bunny", name: "Bunny", env_vars: &["BUNNY_API_KEY"], create: |env| bunny::Bunny::new(env) },
    ProviderMeta { slug: "clouddns", name: "Clouddns", env_vars: &["CLOUDDNS_CLIENT_ID", "CLOUDDNS_EMAIL", "CLOUDDNS_PASSWORD"], create: |env| clouddns::Clouddns::new(env) },
    ProviderMeta { slug: "cloudns", name: "Cloudns", env_vars: &["CLOUDNS_AUTH_ID", "CLOUDNS_AUTH_PASSWORD"], create: |env| cloudns::Cloudns::new(env) },
    ProviderMeta { slug: "cn", name: "Cn", env_vars: &["CN_Username", "CN_Password"], create: |env| cn::Cn::new(env) },
    ProviderMeta { slug: "conoha", name: "Conoha", env_vars: &["CONOHA_Username", "CONOHA_Password", "CONOHA_TenantId", "CONOHA_IdentityServiceApi"], create: |env| conoha::Conoha::new(env) },
    ProviderMeta { slug: "constellix", name: "Constellix", env_vars: &["CONSTELLIX_Key", "CONSTELLIX_Secret"], create: |env| constellix::Constellix::new(env) },
    ProviderMeta { slug: "cpanel", name: "Cpanel", env_vars: &["CPANEL_Hostname", "CPANEL_Username", "CPANEL_ApiToken"], create: |env| cpanel::Cpanel::new(env) },
    ProviderMeta { slug: "curanet", name: "Curanet", env_vars: &["CURANET_AUTH_CLIENT_ID", "CURANET_AUTH_CLIENT_SECRET"], create: |env| curanet::Curanet::new(env) },
    ProviderMeta { slug: "cyon", name: "Cyon", env_vars: &["CY_Username", "CY_Password", "CY_OTP_Secret"], create: |env| cyon::Cyon::new(env) },
    ProviderMeta { slug: "czechia", name: "Czechia", env_vars: &["CZ_AuthorizationToken", "CZ_Zones", "CZ_API_BASE"], create: |env| czechia::Czechia::new(env) },
    ProviderMeta { slug: "da", name: "Da", env_vars: &["DA_Api", "DA_Api_Insecure"], create: |env| da::Da::new(env) },
    ProviderMeta { slug: "ddnss", name: "Ddnss", env_vars: &["DDNSS_Token"], create: |env| ddnss::Ddnss::new(env) },
    ProviderMeta { slug: "desec", name: "Desec", env_vars: &["DESEC_Token"], create: |env| desec::Desec::new(env) },
    ProviderMeta { slug: "df", name: "Df", env_vars: &["DF_USER", "DF_PASSWORD"], create: |env| df::Df::new(env) },
    ProviderMeta { slug: "dgon", name: "Digitalocean", env_vars: &["DO_API_KEY"], create: |env| dgon::Digitalocean::new(env) },
    ProviderMeta { slug: "dnsexit", name: "Dnsexit", env_vars: &["DNSEXIT_API_KEY", "DNSEXIT_AUTH_USER", "DNSEXIT_AUTH_PASS"], create: |env| dnsexit::Dnsexit::new(env) },
    ProviderMeta { slug: "dnshome", name: "Dnshome", env_vars: &["DNSHOME_Username", "DNSHOME_Password"], create: |env| dnshome::Dnshome::new(env) },
    ProviderMeta { slug: "dnsimple", name: "Dnsimple", env_vars: &["DNSimple_OAUTH_TOKEN"], create: |env| dnsimple::Dnsimple::new(env) },
    ProviderMeta { slug: "dnsservices", name: "Dnsservices", env_vars: &["DNSServices_Key", "DNSServices_Secret"], create: |env| dnsservices::Dnsservices::new(env) },
    ProviderMeta { slug: "doapi", name: "Doapi", env_vars: &["DO_API_KEY"], create: |env| doapi::Doapi::new(env) },
    ProviderMeta { slug: "domeneshop", name: "Domeneshop", env_vars: &["DOMENESHOP_Key", "DOMENESHOP_Secret"], create: |env| domeneshop::Domeneshop::new(env) },
    ProviderMeta { slug: "dp", name: "Dnspod", env_vars: &["DP_Id", "DP_Key"], create: |env| dp::Dnspod::new(env) },
    ProviderMeta { slug: "dpi", name: "Dpi", env_vars: &["DP_Id", "DP_Key"], create: |env| dpi::Dpi::new(env) },
    ProviderMeta { slug: "dreamhost", name: "Dreamhost", env_vars: &["DH_API_KEY"], create: |env| dreamhost::Dreamhost::new(env) },
    ProviderMeta { slug: "duckdns", name: "Duckdns", env_vars: &["DuckDNS_Token"], create: |env| duckdns::Duckdns::new(env) },
    ProviderMeta { slug: "durabledns", name: "Durabledns", env_vars: &["DD_API_User", "DD_API_Key"], create: |env| durabledns::Durabledns::new(env) },
    ProviderMeta { slug: "dyn", name: "Dyn", env_vars: &["DYN_Customer", "DYN_Username", "DYN_Password"], create: |env| r#dyn::Dyn::new(env) },
    ProviderMeta { slug: "dynadot", name: "Dynadot", env_vars: &["DYNADOT_Key"], create: |env| dynadot::Dynadot::new(env) },
    ProviderMeta { slug: "dynu", name: "Dynu", env_vars: &["Dynu_ClientId", "Dynu_Secret"], create: |env| dynu::Dynu::new(env) },
    ProviderMeta { slug: "dynv6", name: "Dynv6", env_vars: &["DYNV6_TOKEN"], create: |env| dynv6::Dynv6::new(env) },
    ProviderMeta { slug: "easydns", name: "Easydns", env_vars: &["EASYDNS_Token", "EASYDNS_Key"], create: |env| easydns::Easydns::new(env) },
    ProviderMeta { slug: "edgecenter", name: "Edgecenter", env_vars: &["EDGECENTER_API_KEY"], create: |env| edgecenter::Edgecenter::new(env) },
    ProviderMeta { slug: "edgedns", name: "Edgedns", env_vars: &["AKAMAI_ACCESS_TOKEN", "AKAMAI_CLIENT_TOKEN", "AKAMAI_CLIENT_SECRET", "AKAMAI_HOST", "AKAMAI_EDGERC_CONTENT"], create: |env| edgedns::Edgedns::new(env) },
    ProviderMeta { slug: "efficientip", name: "Efficientip", env_vars: &["EfficientIP_Creds", "EfficientIP_Server", "EfficientIP_Token_Key", "EfficientIP_Token_Secret", "EfficientIP_DNS_Name", "EfficientIP_View"], create: |env| efficientip::Efficientip::new(env) },
    ProviderMeta { slug: "eurodns", name: "Eurodns", env_vars: &["EURODNS_ID", "EURODNS_KEY"], create: |env| eurodns::Eurodns::new(env) },
    ProviderMeta { slug: "euserv", name: "Euserv", env_vars: &["EUSERV_Username", "EUSERV_Password"], create: |env| euserv::Euserv::new(env) },
    ProviderMeta { slug: "exoscale", name: "Exoscale", env_vars: &["EXOSCALE_API_KEY", "EXOSCALE_SECRET_KEY"], create: |env| exoscale::Exoscale::new(env) },
    ProviderMeta { slug: "firestorm", name: "Firestorm", env_vars: &["FST_Key", "FST_Secret", "FST_Url"], create: |env| firestorm::Firestorm::new(env) },
    ProviderMeta { slug: "fornex", name: "Fornex", env_vars: &["FORNEX_API_KEY"], create: |env| fornex::Fornex::new(env) },
    ProviderMeta { slug: "freedns", name: "Freedns", env_vars: &["FREEDNS_User", "FREEDNS_Password"], create: |env| freedns::Freedns::new(env) },
    ProviderMeta { slug: "freemyip", name: "Freemyip", env_vars: &["FREEMYIP_Token"], create: |env| freemyip::Freemyip::new(env) },
    ProviderMeta { slug: "gandi_livedns", name: "GandiLivedns", env_vars: &["GANDI_LIVEDNS_TOKEN"], create: |env| gandi_livedns::GandiLivedns::new(env) },
    ProviderMeta { slug: "gcloud", name: "Gcloud", env_vars: &["GCLOUD_PROJECT", "GCLOUD_SERVICE_ACCOUNT_KEY"], create: |env| gcloud::Gcloud::new(env) },
    ProviderMeta { slug: "gcore", name: "Gcore", env_vars: &["GCORE_PermanentAPIKey"], create: |env| gcore::Gcore::new(env) },
    ProviderMeta { slug: "gd", name: "Godaddy", env_vars: &["GD_Key", "GD_Secret"], create: |env| gd::Godaddy::new(env) },
    ProviderMeta { slug: "geoscaling", name: "Geoscaling", env_vars: &["GEOSCALING_Username", "GEOSCALING_Password"], create: |env| geoscaling::Geoscaling::new(env) },
    ProviderMeta { slug: "gname", name: "Gname", env_vars: &["GNAME_APPID", "GNAME_APPKEY"], create: |env| gname::Gname::new(env) },
    ProviderMeta { slug: "googledomains", name: "Googledomains", env_vars: &["GOOGLEDOMAINS_ACCESS_TOKEN"], create: |env| googledomains::Googledomains::new(env) },
    ProviderMeta { slug: "he", name: "He", env_vars: &["HE_Username", "HE_Password"], create: |env| he::He::new(env) },
    ProviderMeta { slug: "he_ddns", name: "HeDdns", env_vars: &["HE_DDNS_Key", "HE_DDNS_Secret"], create: |env| he_ddns::HeDdns::new(env) },
    ProviderMeta { slug: "hestiacp", name: "Hestiacp", env_vars: &["HESTIACP_USERNAME", "HESTIACP_PASSWORD", "HESTIACP_HOST"], create: |env| hestiacp::Hestiacp::new(env) },
    ProviderMeta { slug: "hetzner", name: "Hetzner", env_vars: &["HETZNER_API_KEY"], create: |env| hetzner::Hetzner::new(env) },
    ProviderMeta { slug: "hetznercloud", name: "Hetznercloud", env_vars: &["HETZNER_TOKEN"], create: |env| hetznercloud::Hetznercloud::new(env) },
    ProviderMeta { slug: "hexonet", name: "Hexonet", env_vars: &["HEXONET_User", "HEXONET_Password"], create: |env| hexonet::Hexonet::new(env) },
    ProviderMeta { slug: "hosting1984", name: "Hosting1984", env_vars: &["One984HOSTING_Username", "One984HOSTING_Password"], create: |env| hosting1984::Hosting1984::new(env) },
    ProviderMeta { slug: "hostingde", name: "Hostingde", env_vars: &["HOSTINGDE_APIKEY", "HOSTINGDE_ENDPOINT"], create: |env| hostingde::Hostingde::new(env) },
    ProviderMeta { slug: "hostingukraine", name: "Hostingukraine", env_vars: &["HOSTINGUKRAINE_UUID", "HOSTINGUKRAINE_TOKEN"], create: |env| hostingukraine::Hostingukraine::new(env) },
    ProviderMeta { slug: "hostline", name: "Hostline", env_vars: &["HOSTLINE_Key", "HOSTLINE_Secret"], create: |env| hostline::Hostline::new(env) },
    ProviderMeta { slug: "hosttech", name: "Hosttech", env_vars: &["HOSTTECH_API_KEY"], create: |env| hosttech::Hosttech::new(env) },
    ProviderMeta { slug: "hostup", name: "Hostup", env_vars: &["HOSTUP_API_KEY"], create: |env| hostup::Hostup::new(env) },
    ProviderMeta { slug: "huaweicloud", name: "Huaweicloud", env_vars: &["HUAWEICLOUD_Username", "HUAWEICLOUD_Password", "HUAWEICLOUD_DomainName"], create: |env| huaweicloud::Huaweicloud::new(env) },
    ProviderMeta { slug: "infoblox", name: "Infoblox", env_vars: &["Infoblox_Creds", "Infoblox_Server", "Infoblox_View"], create: |env| infoblox::Infoblox::new(env) },
    ProviderMeta { slug: "infoblox_uddi", name: "InfobloxUddi", env_vars: &["Infoblox_UDDI_Key", "Infoblox_Portal"], create: |env| infoblox_uddi::InfobloxUddi::new(env) },
    ProviderMeta { slug: "infomaniak", name: "Infomaniak", env_vars: &["INFOMANIAK_ACCESS_TOKEN"], create: |env| infomaniak::Infomaniak::new(env) },
    ProviderMeta { slug: "internetbs", name: "Internetbs", env_vars: &["INTERNETBS_API_KEY", "INTERNETBS_API_PASSWORD"], create: |env| internetbs::Internetbs::new(env) },
    ProviderMeta { slug: "inwx", name: "Inwx", env_vars: &["INWX_User", "INWX_Password"], create: |env| inwx::Inwx::new(env) },
    ProviderMeta { slug: "ionos", name: "Ionos", env_vars: &["IONOS_PREFIX", "IONOS_SECRET", "IONOS_ENDPOINT"], create: |env| ionos::Ionos::new(env) },
    ProviderMeta { slug: "ionos_cloud", name: "IonosCloud", env_vars: &["IONOS_CLOUD_TOKEN"], create: |env| ionos_cloud::IonosCloud::new(env) },
    ProviderMeta { slug: "ipprojects", name: "Ipprojects", env_vars: &["IPP_Apikey"], create: |env| ipprojects::Ipprojects::new(env) },
    ProviderMeta { slug: "ipv64", name: "Ipv64", env_vars: &["IPv64_Token"], create: |env| ipv64::Ipv64::new(env) },
    ProviderMeta { slug: "ispconfig", name: "Ispconfig", env_vars: &["ISPC_User", "ISPC_Password", "ISPC_Api", "ISPC_Api_Insecure"], create: |env| ispconfig::Ispconfig::new(env) },
    ProviderMeta { slug: "ispman", name: "Ispman", env_vars: &["ISPMAN_Url", "ISPMAN_Username", "ISPMAN_Password"], create: |env| ispman::Ispman::new(env) },
    ProviderMeta { slug: "jd", name: "Jd", env_vars: &["JD_ACCESS_KEY_ID", "JD_ACCESS_KEY_SECRET", "JD_REGION"], create: |env| jd::Jd::new(env) },
    ProviderMeta { slug: "joker", name: "Joker", env_vars: &["JOKER_USERNAME", "JOKER_PASSWORD"], create: |env| joker::Joker::new(env) },
    ProviderMeta { slug: "kappernet", name: "Kappernet", env_vars: &["KAPPERNETDNS_Key", "KAPPERNETDNS_Secret"], create: |env| kappernet::Kappernet::new(env) },
    ProviderMeta { slug: "kas", name: "Kas", env_vars: &["KAS_Login", "KAS_Authtype", "KAS_Authdata"], create: |env| kas::Kas::new(env) },
    ProviderMeta { slug: "kinghost", name: "Kinghost", env_vars: &["KINGHOST_Username", "KINGHOST_Password"], create: |env| kinghost::Kinghost::new(env) },
    ProviderMeta { slug: "knot", name: "Knot", env_vars: &["KNOT_SERVER", "KNOT_KEY"], create: |env| knot::Knot::new(env) },
    ProviderMeta { slug: "la", name: "La", env_vars: &["LA_Id", "LA_Sk"], create: |env| la::La::new(env) },
    ProviderMeta { slug: "leaseweb", name: "Leaseweb", env_vars: &["LSW_Key"], create: |env| leaseweb::Leaseweb::new(env) },
    ProviderMeta { slug: "lexicon", name: "Lexicon", env_vars: &["LEXICON_Provider"], create: |env| lexicon::Lexicon::new(env) },
    ProviderMeta { slug: "limacity", name: "Limacity", env_vars: &["LIMACITY_APIKEY"], create: |env| limacity::Limacity::new(env) },
    ProviderMeta { slug: "linode", name: "Linode", env_vars: &["LINODE_API_KEY"], create: |env| linode::Linode::new(env) },
    ProviderMeta { slug: "linode_v4", name: "Linodev4", env_vars: &["LINODE_V4_API_KEY"], create: |env| linode_v4::Linodev4::new(env) },
    ProviderMeta { slug: "loopia", name: "Loopia", env_vars: &["LOOPIA_User", "LOOPIA_Password"], create: |env| loopia::Loopia::new(env) },
    ProviderMeta { slug: "lua", name: "Luadns", env_vars: &["LUA_Key", "LUA_Email"], create: |env| lua::Luadns::new(env) },
    ProviderMeta { slug: "maradns", name: "Maradns", env_vars: &["MARA_ZONE_FILE", "MARA_DUENDE_PID_PATH"], create: |env| maradns::Maradns::new(env) },
    ProviderMeta { slug: "me", name: "Dnsmadeeasy", env_vars: &["ME_Key", "ME_Secret"], create: |env| me::Dnsmadeeasy::new(env) },
    ProviderMeta { slug: "mgwm", name: "Mgwm", env_vars: &["MGWM_CUSTOMER", "MGWM_API_HASH"], create: |env| mgwm::Mgwm::new(env) },
    ProviderMeta { slug: "miab", name: "Miab", env_vars: &["MIAB_Username", "MIAB_Password", "MIAB_Server"], create: |env| miab::Miab::new(env) },
    ProviderMeta { slug: "mijnhost", name: "Mijnhost", env_vars: &["MIJNHOST_API_KEY"], create: |env| mijnhost::Mijnhost::new(env) },
    ProviderMeta { slug: "misaka", name: "Misaka", env_vars: &["Misaka_Key"], create: |env| misaka::Misaka::new(env) },
    ProviderMeta { slug: "myapi", name: "Myapi", env_vars: &["MYAPI_Token", "MYAPI_Endpoint"], create: |env| myapi::Myapi::new(env) },
    ProviderMeta { slug: "mydevil", name: "Mydevil", env_vars: &["MYDEVIL_Username", "MYDEVIL_Password"], create: |env| mydevil::Mydevil::new(env) },
    ProviderMeta { slug: "mydnsjp", name: "Mydnsjp", env_vars: &["MYDNSJP_MasterID", "MYDNSJP_MasterPassword"], create: |env| mydnsjp::Mydnsjp::new(env) },
    ProviderMeta { slug: "myloc", name: "Myloc", env_vars: &["MYLOC_User", "MYLOC_Password"], create: |env| myloc::Myloc::new(env) },
    ProviderMeta { slug: "mythic_beasts", name: "MythicBeasts", env_vars: &["MB_AK", "MB_AS"], create: |env| mythic_beasts::MythicBeasts::new(env) },
    ProviderMeta { slug: "namecheap", name: "Namecheap", env_vars: &["NAMECHEAP_API_KEY", "NAMECHEAP_USERNAME"], create: |env| namecheap::Namecheap::new(env) },
    ProviderMeta { slug: "namecom", name: "Namecom", env_vars: &["Namecom_Username", "Namecom_Token"], create: |env| namecom::Namecom::new(env) },
    ProviderMeta { slug: "namesilo", name: "Namesilo", env_vars: &["Namesilo_Key"], create: |env| namesilo::Namesilo::new(env) },
    ProviderMeta { slug: "nanelo", name: "Nanelo", env_vars: &["NANELO_TOKEN"], create: |env| nanelo::Nanelo::new(env) },
    ProviderMeta { slug: "nederhost", name: "Nederhost", env_vars: &["NederHost_Key"], create: |env| nederhost::Nederhost::new(env) },
    ProviderMeta { slug: "neodigit", name: "Neodigit", env_vars: &["NEODIGIT_API_TOKEN"], create: |env| neodigit::Neodigit::new(env) },
    ProviderMeta { slug: "netcup", name: "Netcup", env_vars: &["NETCUP_CUSTOMER_NUMBER", "NETCUP_API_KEY", "NETCUP_API_PASSWORD"], create: |env| netcup::Netcup::new(env) },
    ProviderMeta { slug: "netim", name: "Netim", env_vars: &["NETIM_Username", "NETIM_Password"], create: |env| netim::Netim::new(env) },
    ProviderMeta { slug: "netlify", name: "Netlify", env_vars: &["NETLIFY_ACCESS_TOKEN"], create: |env| netlify::Netlify::new(env) },
    ProviderMeta { slug: "nic", name: "Nic", env_vars: &["NIC_ClientID", "NIC_ClientSecret", "NIC_Username", "NIC_Password"], create: |env| nic::Nic::new(env) },
    ProviderMeta { slug: "njalla", name: "Njalla", env_vars: &["NJALLA_Token"], create: |env| njalla::Njalla::new(env) },
    ProviderMeta { slug: "nm", name: "Nm", env_vars: &["NM_user", "NM_sha256"], create: |env| nm::Nm::new(env) },
    ProviderMeta { slug: "nodion", name: "Nodion", env_vars: &["NODION_API_KEY"], create: |env| nodion::Nodion::new(env) },
    ProviderMeta { slug: "nsd", name: "Nsd", env_vars: &["NSD_SERVER", "NSD_KEY"], create: |env| nsd::Nsd::new(env) },
    ProviderMeta { slug: "nsone", name: "Nsone", env_vars: &["NS1_Key"], create: |env| nsone::Nsone::new(env) },
    ProviderMeta { slug: "nsupdate", name: "Nsupdate", env_vars: &["NSUPDATE_SERVER", "NSUPDATE_KEY"], create: |env| nsupdate::Nsupdate::new(env) },
    ProviderMeta { slug: "nw", name: "Nw", env_vars: &["NW_API_TOKEN", "NW_API_ENDPOINT"], create: |env| nw::Nw::new(env) },
    ProviderMeta { slug: "oci", name: "Oci", env_vars: &["OCI_PRIVKEY", "OCI_TENANCY", "OCI_USER", "OCI_REGION"], create: |env| oci::Oci::new(env) },
    ProviderMeta { slug: "omglol", name: "Omglol", env_vars: &["OMG_ApiKey", "OMG_Address"], create: |env| omglol::Omglol::new(env) },
    ProviderMeta { slug: "one", name: "One", env_vars: &["ONE_Username", "ONE_Password"], create: |env| one::One::new(env) },
    ProviderMeta { slug: "online", name: "Online", env_vars: &["ONLINE_API_KEY"], create: |env| online::Online::new(env) },
    ProviderMeta { slug: "openprovider", name: "Openprovider", env_vars: &["OPENPROVIDER_USER", "OPENPROVIDER_PASSWORD_HASH"], create: |env| openprovider::Openprovider::new(env) },
    ProviderMeta { slug: "openprovider_rest", name: "OpenproviderRest", env_vars: &["OPENPROVIDER_REST_USERNAME", "OPENPROVIDER_REST_PASSWORD"], create: |env| openprovider_rest::OpenproviderRest::new(env) },
    ProviderMeta { slug: "openstack", name: "Openstack", env_vars: &["OS_AUTH_URL", "OS_USERNAME", "OS_PASSWORD", "OS_PROJECT_NAME", "OS_PROJECT_DOMAIN_NAME", "OS_USER_DOMAIN_NAME", "OS_AUTH_TYPE", "OS_APPLICATION_CREDENTIAL_ID", "OS_APPLICATION_CREDENTIAL_SECRET", "OS_PROJECT_ID", "OS_USER_DOMAIN_ID", "OS_PROJECT_DOMAIN_ID"], create: |env| openstack::Openstack::new(env) },
    ProviderMeta { slug: "opnsense", name: "Opnsense", env_vars: &["OPNSENSE_API_KEY", "OPNSENSE_API_SECRET", "OPNSENSE_HOST"], create: |env| opnsense::Opnsense::new(env) },
    ProviderMeta { slug: "opusdns", name: "Opusdns", env_vars: &["OPUSDNS_API_Key", "OPUSDNS_API_Endpoint", "OPUSDNS_TTL"], create: |env| opusdns::Opusdns::new(env) },
    ProviderMeta { slug: "ovh", name: "Ovh", env_vars: &["OVH_AK", "OVH_AS", "OVH_CK", "OVH_END_POINT"], create: |env| ovh::Ovh::new(env) },
    ProviderMeta { slug: "pdns", name: "Powerdns", env_vars: &["PDNS_Url", "PDNS_ServerId", "PDNS_Token", "PDNS_Ttl"], create: |env| pdns::Powerdns::new(env) },
    ProviderMeta { slug: "pdnsmanager", name: "Pdnsmanager", env_vars: &["PDNSMGR_API_KEY", "PDNSMGR_API_PASSWORD", "PDNSMGR_API_URL"], create: |env| pdnsmanager::Pdnsmanager::new(env) },
    ProviderMeta { slug: "pleskxml", name: "Pleskxml", env_vars: &["pleskxml_user", "pleskxml_pass", "pleskxml_uri"], create: |env| pleskxml::Pleskxml::new(env) },
    ProviderMeta { slug: "pmiab", name: "Pmiab", env_vars: &["PMIAB_Username", "PMIAB_Password", "PMIAB_Server"], create: |env| pmiab::Pmiab::new(env) },
    ProviderMeta { slug: "pointhq", name: "Pointhq", env_vars: &["PointHQ_Key", "PointHQ_Email"], create: |env| pointhq::Pointhq::new(env) },
    ProviderMeta { slug: "porkbun", name: "Porkbun", env_vars: &["PORKBUN_API_KEY", "PORKBUN_SECRET_API_KEY"], create: |env| porkbun::Porkbun::new(env) },
    ProviderMeta { slug: "poweradmin", name: "Poweradmin", env_vars: &["POWERADMIN_URL", "POWERADMIN_API_KEY", "POWERADMIN_API_VERSION"], create: |env| poweradmin::Poweradmin::new(env) },
    ProviderMeta { slug: "qc", name: "Qc", env_vars: &["QC_API_KEY", "QC_API_EMAIL"], create: |env| qc::Qc::new(env) },
    ProviderMeta { slug: "rackcorp", name: "Rackcorp", env_vars: &["RACKCORP_APIUUID", "RACKCORP_APISECRET"], create: |env| rackcorp::Rackcorp::new(env) },
    ProviderMeta { slug: "rackspace", name: "Rackspace", env_vars: &["RACKSPACE_Username", "RACKSPACE_ApiKey"], create: |env| rackspace::Rackspace::new(env) },
    ProviderMeta { slug: "rage4", name: "Rage4", env_vars: &["RAGE4_Key", "RAGE4_Secret"], create: |env| rage4::Rage4::new(env) },
    ProviderMeta { slug: "rcode0", name: "Rcode0", env_vars: &["RCODE0_API_TOKEN", "RCODE0_URL", "RCODE0_TTL"], create: |env| rcode0::Rcode0::new(env) },
    ProviderMeta { slug: "regru", name: "Regru", env_vars: &["REGRU_Username", "REGRU_Password"], create: |env| regru::Regru::new(env) },
    ProviderMeta { slug: "restena", name: "Restena", env_vars: &["RESTENA_Username", "RESTENA_Password"], create: |env| restena::Restena::new(env) },
    ProviderMeta { slug: "samba", name: "Samba", env_vars: &["SAMBA_HOSTNAME", "SAMBA_DOMAIN", "SAMBA_USERNAME", "SAMBA_PASSWORD"], create: |env| samba::Samba::new(env) },
    ProviderMeta { slug: "scaleway", name: "Scaleway", env_vars: &["SCALEWAY_API_TOKEN", "SCALEWAY_PROJECT_ID"], create: |env| scaleway::Scaleway::new(env) },
    ProviderMeta { slug: "schlundtech", name: "Schlundtech", env_vars: &["SCHLUNDTECH_USER", "SCHLUNDTECH_PASSWORD"], create: |env| schlundtech::Schlundtech::new(env) },
    ProviderMeta { slug: "sdns", name: "Sdns", env_vars: &["SDNS_Username", "SDNS_Password"], create: |env| sdns::Sdns::new(env) },
    ProviderMeta { slug: "selectel", name: "Selectel", env_vars: &["SL_Ver", "SL_Key", "SL_Login_ID", "SL_Project_Name", "SL_Login_Name", "SL_Pswd", "SL_Expire"], create: |env| selectel::Selectel::new(env) },
    ProviderMeta { slug: "selfhost", name: "Selfhost", env_vars: &["SELFHOSTDNS_USERNAME", "SELFHOSTDNS_PASSWORD", "SELFHOSTDNS_MAP"], create: |env| selfhost::Selfhost::new(env) },
    ProviderMeta { slug: "shellrent", name: "Shellrent", env_vars: &["SHELLRENT_Username", "SHELLRENT_Password"], create: |env| shellrent::Shellrent::new(env) },
    ProviderMeta { slug: "simply", name: "Simply", env_vars: &["SIMPLY_ApiLogin", "SIMPLY_ApiKey"], create: |env| simply::Simply::new(env) },
    ProviderMeta { slug: "sitehost", name: "Sitehost", env_vars: &["SITEHOST_API_KEY", "SITEHOST_CLIENT_ID"], create: |env| sitehost::Sitehost::new(env) },
    ProviderMeta { slug: "sotoon", name: "Sotoon", env_vars: &["Sotoon_Token", "Sotoon_WorkspaceUUID"], create: |env| sotoon::Sotoon::new(env) },
    ProviderMeta { slug: "spaceship", name: "Spaceship", env_vars: &["SPACESHIP_API_KEY", "SPACESHIP_API_SECRET", "SPACESHIP_ROOT_DOMAIN"], create: |env| spaceship::Spaceship::new(env) },
    ProviderMeta { slug: "subreg", name: "Subreg", env_vars: &["SUBREG_API_USERNAME", "SUBREG_API_PASSWORD"], create: |env| subreg::Subreg::new(env) },
    ProviderMeta { slug: "synology_dsm", name: "SynologyDsm", env_vars: &["SYNOLOGY_DSM_HOSTNAME", "SYNOLOGY_DSM_USERNAME", "SYNOLOGY_DSM_PASSWORD"], create: |env| synology_dsm::SynologyDsm::new(env) },
    ProviderMeta { slug: "technitium", name: "Technitium", env_vars: &["TECHNITIUM_Server", "TECHNITIUM_Token"], create: |env| technitium::Technitium::new(env) },
    ProviderMeta { slug: "tele3", name: "Tele3", env_vars: &["TELE3_Key", "TELE3_Secret"], create: |env| tele3::Tele3::new(env) },
    ProviderMeta { slug: "tencent", name: "Tencent", env_vars: &["TENCENT_SecretId", "TENCENT_SecretKey"], create: |env| tencent::Tencent::new(env) },
    ProviderMeta { slug: "timeweb", name: "Timeweb", env_vars: &["TIMEWEB_Token"], create: |env| timeweb::Timeweb::new(env) },
    ProviderMeta { slug: "transip", name: "Transip", env_vars: &["TRANSIP_Username", "TRANSIP_Key"], create: |env| transip::Transip::new(env) },
    ProviderMeta { slug: "udr", name: "Udr", env_vars: &["UDR_USER", "UDR_PASS"], create: |env| udr::Udr::new(env) },
    ProviderMeta { slug: "ultra", name: "Ultra", env_vars: &["ULTRA_USR", "ULTRA_PWD"], create: |env| ultra::Ultra::new(env) },
    ProviderMeta { slug: "unoeuro", name: "Unoeuro", env_vars: &["UNOEURO_User", "UNOEURO_Password"], create: |env| unoeuro::Unoeuro::new(env) },
    ProviderMeta { slug: "variomedia", name: "Variomedia", env_vars: &["VARIOMEDIA_API_TOKEN"], create: |env| variomedia::Variomedia::new(env) },
    ProviderMeta { slug: "veesp", name: "Veesp", env_vars: &["VEESP_User", "VEESP_Password"], create: |env| veesp::Veesp::new(env) },
    ProviderMeta { slug: "vercel", name: "Vercel", env_vars: &["VERCEL_TOKEN"], create: |env| vercel::Vercel::new(env) },
    ProviderMeta { slug: "virakcloud", name: "Virakcloud", env_vars: &["VIRAKCLOUD_API_TOKEN"], create: |env| virakcloud::Virakcloud::new(env) },
    ProviderMeta { slug: "vscale", name: "Vscale", env_vars: &["VSCALE_API_KEY"], create: |env| vscale::Vscale::new(env) },
    ProviderMeta { slug: "vultr", name: "Vultr", env_vars: &["VULTR_API_KEY"], create: |env| vultr::Vultr::new(env) },
    ProviderMeta { slug: "websupport", name: "Websupport", env_vars: &["WS_ApiKey", "WS_ApiSecret"], create: |env| websupport::Websupport::new(env) },
    ProviderMeta { slug: "wedos", name: "Wedos", env_vars: &["WEDOS_User", "WEDOS_Password"], create: |env| wedos::Wedos::new(env) },
    ProviderMeta { slug: "west_cn", name: "WestCn", env_vars: &["WEST_Username", "WEST_Key"], create: |env| west_cn::WestCn::new(env) },
    ProviderMeta { slug: "wexbo", name: "Wexbo", env_vars: &["WEXBO_User", "WEXBO_Password"], create: |env| wexbo::Wexbo::new(env) },
    ProviderMeta { slug: "world4you", name: "World4you", env_vars: &["WORLD4YOU_Username", "WORLD4YOU_Password"], create: |env| world4you::World4you::new(env) },
    ProviderMeta { slug: "wts", name: "Wts", env_vars: &["WTS_Key", "WTS_Secret"], create: |env| wts::Wts::new(env) },
    ProviderMeta { slug: "yandex", name: "Yandex", env_vars: &["YANDEX_Token"], create: |env| yandex::Yandex::new(env) },
    ProviderMeta { slug: "yandex360", name: "Yandex360", env_vars: &["YANDEX360_ACCESS_TOKEN"], create: |env| yandex360::Yandex360::new(env) },
    ProviderMeta { slug: "yc", name: "Yc", env_vars: &["YC_SA_ID", "YC_SA_Key_ID", "YC_SA_Key_File_Path", "YC_Folder_ID", "YC_Zone_ID"], create: |env| yc::Yc::new(env) },
    ProviderMeta { slug: "zilore", name: "Zilore", env_vars: &["Zilore_Key"], create: |env| zilore::Zilore::new(env) },
    ProviderMeta { slug: "zone", name: "Zone", env_vars: &["ZONE_Username", "ZONE_Key"], create: |env| zone::Zone::new(env) },
    ProviderMeta { slug: "zoneedit", name: "Zoneedit", env_vars: &["ZONEEDIT_ID", "ZONEEDIT_Token"], create: |env| zoneedit::Zoneedit::new(env) },
    ProviderMeta { slug: "zonomi", name: "Zonomi", env_vars: &["ZM_Key"], create: |env| zonomi::Zonomi::new(env) },
];

pub fn find(slug: &str) -> Option<&'static ProviderMeta> {
    PROVIDERS.iter().find(|p| p.slug == slug)
}

pub fn list() -> Vec<&'static ProviderMeta> {
    PROVIDERS.iter().collect()
}
