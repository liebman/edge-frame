use std::net::Ipv4Addr;
use std::str::FromStr;

use enumset::EnumSet;

use strum::*;

use log::info;

use yew::prelude::*;

use embedded_svc::ipv4::{self, DHCPClientSettings, RouterConfiguration, Subnet};
use embedded_svc::utils::rest::role::Role;
use embedded_svc::wifi::{
    AccessPointConfiguration, AuthMethod, ClientConfiguration, Configuration,
};

use crate::api::wifi::WifiEndpoint;
use crate::plugin::*;
use crate::utils::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Hash))]
pub enum PluginBehavior {
    STA,
    AP,
    Mixed,
}

pub fn plugin(behavior: PluginBehavior) -> SimplePlugin<bool> {
    SimplePlugin {
        name: "Wifi".into(),
        description: Some(
            match behavior {
                PluginBehavior::STA => "A settings user interface for configuring Wifi access",
                PluginBehavior::AP => "A settings user interface for configuring Wifi Access Point",
                PluginBehavior::Mixed => {
                    "A settings user interface for configuring WiFi Access Point and STA"
                }
            }
            .into(),
        ),
        icon: Some("fa-lg fa-solid fa-wifi".into()),
        min_role: Role::Admin,
        insertion_points: EnumSet::only(InsertionPoint::Navigation)
            .union(EnumSet::only(InsertionPoint::Status)),
        category: Category::Settings,
        route: true,
        component: Callback2::from(move |plugin_props: PluginProps<bool>| {
            html! {
                <Wifi behavior={behavior} endpoint={plugin_props.api_endpoint}/>
            }
        }),
    }
}

#[derive(Properties, Clone, Debug, PartialEq)]
pub struct WifiProps {
    pub behavior: PluginBehavior,
    pub endpoint: Option<APIEndpoint>,
}

#[function_component(Wifi)]
pub fn wifi(props: &WifiProps) -> Html {
    let api = WifiEndpoint {};

    let status_state = use_state_eq(|| None);
    let conf_state: UseStateHandle<Option<Configuration>> = use_state_eq(|| None);

    let ap_conf_form = ApConfForm::new();
    let sta_conf_form = StaConfForm::new();

    {
        let api = api.clone();
        let status_state = status_state.clone();

        use_effect_with_deps(
            |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    loop {
                        let status = api.get_status().await.unwrap();
                        info!("Got status {:?}", status);

                        status_state.set(Some(status));
                    }
                });

                || ()
            },
            (),
        );
    }

    {
        let api = api.clone();
        let conf_state = conf_state.clone();
        let ap_conf_form = ap_conf_form.clone();
        let sta_conf_form = sta_conf_form.clone();

        use_effect_with_deps(
            |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    //loop {
                    let conf = api.get_configuration().await.unwrap();
                    info!("Got conf {:?}", conf);

                    if conf_state.as_ref() != Some(&conf) {
                        ap_conf_form.set(conf.as_ap_conf_ref().unwrap_or(&Default::default()));
                        sta_conf_form.set(conf.as_client_conf_ref().unwrap_or(&Default::default()));

                        conf_state.set(Some(conf));
                    }
                    //}
                });

                || ()
            },
            (),
        );
    }

    let onclick = {
        let api = api.clone();
        let ap_conf_form = ap_conf_form.clone();
        let sta_conf_form = sta_conf_form.clone();

        Callback::from(move |_| {
            let mut conf = Configuration::Mixed(Default::default(), Default::default());

            if let Some(ap_conf) = ap_conf_form.get() {
                *conf.as_ap_conf_mut() = ap_conf;
            }

            if let Some(sta_conf) = sta_conf_form.get() {
                *conf.as_client_conf_mut() = sta_conf;
            }

            let mut api = api.clone();

            wasm_bindgen_futures::spawn_local(async move {
                api.set_configuration(&conf).await.unwrap();
            });
        })
    };

    let mobile = true;

    let ap_active = use_state(|| true);
    let switch = {
        let ap_active = ap_active.clone();
        Callback::from(move |_| ap_active.set(!*ap_active))
    };

    html! {
        <>
        <div class="container">
        {
            if mobile {
                html! {
                    <>
                    <div class="tabs">
                        <ul>
                            <li class={if_true(*ap_active, "is-active")}>
                                <a class={if_true(ap_conf_form.has_errors(), "has-text-danger")} href="#" onclick={switch.clone()}>{"Access Point"}</a>
                            </li>
                            <li class={if_true(!*ap_active, "is-active")}>
                                <a class={if_true(sta_conf_form.has_errors(), "has-text-danger")} href="#" onclick={switch}>{"Client"}</a>
                            </li>
                        </ul>
                    </div>
                    <div>
                        {
                            if *ap_active {
                                ap_conf_form.render(conf_state.is_none())
                            } else {
                                sta_conf_form.render(conf_state.is_none())
                            }
                        }
                    </div>
                    </>
                }
            } else {
                html! {
                    <div class="tile is-ancestor">
                        <div class="tile is-4 is-vertical is-parent">
                            <div class="tile is-child box">
                                <p class={classes!("title", if_true(ap_conf_form.has_errors(), "is-danger"))}>{"Access Point"}</p>
                                {ap_conf_form.render(conf_state.is_none())}
                            </div>
                        </div>
                        <div class="tile is-4 is-vertical is-parent">
                            <div class="tile is-child box">
                                <p class={classes!("title", if_true(sta_conf_form.has_errors(), "is-danger"))}>{"Client"}</p>
                                {sta_conf_form.render(conf_state.is_none())}
                            </div>
                        </div>
                    </div>
                }
            }
        }

        <input
            type="button"
            class={"button my-4"}
            value="Save"
            disabled={
                conf_state.is_none()
                || ap_conf_form.has_errors()
                || sta_conf_form.has_errors()
                || conf_state.as_ref().and_then(|conf| conf.as_ap_conf_ref()) == ap_conf_form.get().as_ref()
                    && conf_state.as_ref().and_then(|conf| conf.as_client_conf_ref()) == sta_conf_form.get().as_ref()
            }
            {onclick}
        />
        </div>
        </>
    }
}

#[derive(Clone)]
struct ApConfForm {
    ssid: TextField<String>,
    hidden_ssid: CheckedField<bool>,

    auth: TextField<AuthMethod>,
    password: TextField<String>,
    password_confirm: TextField<String>,

    ip_conf_enabled: CheckedField<bool>,
    dhcp_server_enabled: CheckedField<bool>,
    subnet: TextField<Subnet>,
    dns: TextField<Option<Ipv4Addr>>,
    secondary_dns: TextField<Option<Ipv4Addr>>,
}

impl ApConfForm {
    fn new() -> Self {
        let password = Field::text(|password| {
            if password.is_empty() {
                Err("Password cannot be empty".into())
            } else {
                Ok(password)
            }
        });

        Self {
            ssid: Field::text(Ok),
            hidden_ssid: Field::checked(Ok),
            auth: Field::text(|raw_value| {
                Ok(AuthMethod::iter()
                    .find(|auth| auth.to_string() == raw_value)
                    .unwrap_or(Default::default()))
            }),
            password: password.clone(),
            password_confirm: Field::text(move |raw_text| {
                if raw_text == password.raw_value() {
                    Ok(raw_text)
                } else {
                    Err("Passwords do not match".into())
                }
            }),
            ip_conf_enabled: Field::checked(Ok),
            dhcp_server_enabled: Field::checked(Ok),
            subnet: Field::text(|raw_text| Subnet::from_str(&raw_text).map_err(str::to_owned)),
            dns: TextField::<Option<Ipv4Addr>>::text(|raw_value| {
                if raw_value.trim().is_empty() {
                    Ok(None)
                } else {
                    Ipv4Addr::from_str(&raw_value).map(Some).map_err(|_| {
                        "Invalid IP address format, expected XXX.XXX.XXX.XXX".to_owned()
                    })
                }
            }),
            secondary_dns: TextField::<Option<Ipv4Addr>>::text(|raw_value| {
                if raw_value.trim().is_empty() {
                    Ok(None)
                } else {
                    Ipv4Addr::from_str(&raw_value).map(Some).map_err(|_| {
                        "Invalid IP address format, expected XXX.XXX.XXX.XXX".to_owned()
                    })
                }
            }),
        }
    }

    fn has_errors(&self) -> bool {
        self.ssid.has_errors()
            || self.hidden_ssid.has_errors()
            || self.auth.has_errors()
            || self.auth.value() != Some(AuthMethod::None)
                && (self.password.has_errors() || self.password_confirm.has_errors())
            || self.ip_conf_enabled.has_errors()
            || self.ip_conf_enabled.value() == Some(true)
                && (self.dhcp_server_enabled.has_errors()
                    || self.subnet.has_errors()
                    || self.dns.has_errors()
                    || self.secondary_dns.has_errors())
    }

    fn get(&self) -> Option<AccessPointConfiguration> {
        if self.has_errors() {
            None
        } else {
            Some(AccessPointConfiguration {
                ssid: self.ssid.value().unwrap(),
                ssid_hidden: self.hidden_ssid.value().unwrap(),

                auth_method: self.auth.value().unwrap(),
                password: self.password.value().unwrap_or_else(|| String::new()),

                ip_conf: if self.ip_conf_enabled.value().unwrap() {
                    Some(RouterConfiguration {
                        dhcp_enabled: self.dhcp_server_enabled.value().unwrap(),
                        subnet: self.subnet.value().unwrap(),
                        dns: self.dns.value().unwrap(),
                        secondary_dns: self.secondary_dns.value().unwrap(),
                    })
                } else {
                    None
                },

                ..Default::default()
            })
        }
    }

    fn set(&self, conf: &AccessPointConfiguration) {
        self.ssid.set(conf.ssid.clone());
        self.hidden_ssid.set(conf.ssid_hidden);

        self.auth.set(conf.auth_method.to_string());
        self.password.set(conf.password.clone());
        self.password_confirm.set(conf.password.clone());

        self.ip_conf_enabled.set(conf.ip_conf.is_some());

        self.dhcp_server_enabled
            .set(conf.ip_conf.map(|i| i.dhcp_enabled).unwrap_or(false));
        self.subnet.set(
            conf.ip_conf
                .map(|i| i.subnet.to_string())
                .unwrap_or_else(|| String::new()),
        );
        self.dns.set(
            conf.ip_conf
                .and_then(|i| i.dns.map(|d| d.to_string()))
                .unwrap_or_else(|| String::new()),
        );
        self.secondary_dns.set(
            conf.ip_conf
                .and_then(|i| i.secondary_dns.map(|d| d.to_string()))
                .unwrap_or_else(|| String::new()),
        );
    }

    fn render(&self, disabled: bool) -> Html {
        let disabled_ip = disabled || !self.ip_conf_enabled.value().unwrap_or(false);

        let hidden = if_true(disabled, "visibility: hidden;");
        let hidden_ip = if_true(disabled_ip, "visibility: hidden;");

        let input_class = |errors| classes!("input", if_true(!disabled && errors, "is-danger"));
        let input_class_ip =
            |errors| classes!("input", if_true(!disabled_ip && errors, "is-danger"));

        html! {
            <>
            // SSID
            <div class="field">
                <label class="label">{ "SSID" }</label>
                <div class="control">
                    <input
                        class={input_class(self.ssid.has_errors())}
                        type="text"
                        placeholder="0..24 characters"
                        value={self.ssid.raw_value()}
                        {disabled}
                        oninput={self.ssid.change()}
                        />
                </div>
                <p class="help is-danger" style={hidden}>{self.ssid.error_str()}</p>
            </div>

            // Hide SSID
            <div class="field">
                <label class="checkbox" {disabled}>
                    <input
                        type="checkbox"
                        checked={self.hidden_ssid.raw_value()}
                        {disabled}
                        onclick={self.hidden_ssid.change()}
                    />
                    {"Hidden"}
                </label>
            </div>

            // Authentication
            <div class="field">
                <label class="label">{"Authentication"}</label>
                <div class="control">
                    <div class="select">
                        <select disabled={disabled} onchange={self.auth.change()}>
                        {
                            AuthMethod::iter().map(|item| {
                                html! {
                                    <option value={item.to_string()} selected={Some(item) == self.auth.value()}>
                                        {item.get_message().map(str::to_owned).unwrap_or_else(|| item.to_string())}
                                    </option>
                                }
                            })
                            .collect::<Html>()
                        }
                        </select>
                    </div>
                </div>
            </div>

            {
                if self.auth.value() != Some(AuthMethod::None) {
                    html! {
                        <>
                        // Password
                        <div class="field">
                            <label class="label">{if self.auth.value() == Some(AuthMethod::WEP) { "Key" } else { "Password" }}</label>
                            <div class="control">
                                <input
                                    class={input_class(self.password.has_errors())}
                                    type="password"
                                    placeholder="0..24 characters"
                                    value={self.password.raw_value()}
                                    disabled={disabled}
                                    oninput={self.password.change()}
                                    />
                            </div>
                            <p class="help is-danger" style={hidden}>{self.password.error_str()}</p>
                        </div>

                        // Confirm password
                        <div class="field">
                            <label class="label">{if self.auth.value() == Some(AuthMethod::WEP) { "Key Confirmation" } else { "Password Confirmation" }}</label>
                            <div class="control">
                                <input
                                    class={input_class(self.password_confirm.has_errors())}
                                    type="password"
                                    placeholder="0..24 characters"
                                    value={self.password_confirm.raw_value()}
                                    disabled={disabled}
                                    oninput={self.password_confirm.change()}
                                    />
                            </div>
                            <p class="help is-danger" style={hidden}>{self.password_confirm.error_str()}</p>
                        </div>
                        </>
                    }
                } else {
                    html! {}
                }
            }

            // IP Configuration
            <div class="field">
                <label class="checkbox" {disabled}>
                    <input
                        type="checkbox"
                        checked={self.ip_conf_enabled.raw_value()}
                        {disabled}
                        onclick={self.ip_conf_enabled.change()}
                    />
                    {"IP Configuration"}
                </label>
            </div>

            // DHCP Server
            <div class="field">
                <label class="checkbox" disabled={disabled_ip}>
                    <input
                        type="checkbox"
                        checked={self.dhcp_server_enabled.raw_value()}
                        disabled={disabled_ip}
                        onclick={self.dhcp_server_enabled.change()}
                    />
                    {"DHCP Server"}
                </label>
            </div>

            // Subnet
            <div class="field">
                <label class="label">{ "Subnet" }</label>
                <div class="control">
                    <input
                        class={input_class_ip(self.subnet.has_errors())}
                        type="text"
                        placeholder="XXX.XXX.XXX.XXX/YY"
                        value={self.subnet.raw_value()}
                        disabled={disabled_ip}
                        oninput={self.subnet.change()}
                        />
                </div>
                <p class="help is-danger" style={hidden_ip}>{self.subnet.error_str()}</p>
            </div>

            // DNS
            <div class="field">
                <label class="label">{ "DNS" }</label>
                <div class="control">
                    <input
                        class={input_class_ip(self.dns.has_errors())}
                        type="text"
                        placeholder="XXX.XXX.XXX.XXX (optional)"
                        value={self.dns.raw_value()}
                        disabled={disabled_ip}
                        oninput={self.dns.change()}
                        />
                </div>
                <p class="help is-danger" style={hidden_ip}>{self.dns.error_str()}</p>
            </div>

            // Secondary DNS
            <div class="field">
                <label class="label">{ "Secondary DNS" }</label>
                <div class="control">
                    <input
                        class={input_class_ip(self.secondary_dns.has_errors())}
                        type="text"
                        placeholder="XXX.XXX.XXX.XXX (optional)"
                        value={self.secondary_dns.raw_value()}
                        disabled={disabled_ip}
                        oninput={self.secondary_dns.change()}
                        />
                </div>
                <p class="help is-danger" style={hidden_ip}>{self.secondary_dns.error_str()}</p>
            </div>
            </>
        }
    }
}

#[derive(Clone)]
struct StaConfForm {
    ssid: TextField<String>,

    auth: TextField<AuthMethod>,
    password: TextField<String>,
    password_confirm: TextField<String>,

    ip_conf_enabled: CheckedField<bool>,
    dhcp_enabled: CheckedField<bool>,
    subnet: TextField<Subnet>,
    ip: TextField<Ipv4Addr>,
    dns: TextField<Option<Ipv4Addr>>,
    secondary_dns: TextField<Option<Ipv4Addr>>,
}

impl StaConfForm {
    fn new() -> Self {
        let password = Field::text(|password| {
            if password.is_empty() {
                Err("Password cannot be empty".into())
            } else {
                Ok(password)
            }
        });

        Self {
            ssid: Field::text(Ok),
            auth: Field::text(|raw_value| {
                Ok(AuthMethod::iter()
                    .find(|auth| auth.to_string() == raw_value)
                    .unwrap_or(Default::default()))
            }),
            password: password.clone(),
            password_confirm: Field::text(move |raw_text| {
                if raw_text == password.raw_value() {
                    Ok(raw_text)
                } else {
                    Err("Passwords do not match".into())
                }
            }),
            ip_conf_enabled: Field::checked(Ok),
            dhcp_enabled: Field::checked(Ok),
            subnet: Field::text(|raw_text| Subnet::from_str(&raw_text).map_err(str::to_owned)),
            ip: TextField::<Ipv4Addr>::text(|raw_value| {
                Ipv4Addr::from_str(&raw_value)
                    .map_err(|_| "Invalid IP address format, expected XXX.XXX.XXX.XXX".to_owned())
            }),
            dns: TextField::<Option<Ipv4Addr>>::text(|raw_value| {
                if raw_value.trim().is_empty() {
                    Ok(None)
                } else {
                    Ipv4Addr::from_str(&raw_value).map(Some).map_err(|_| {
                        "Invalid IP address format, expected XXX.XXX.XXX.XXX".to_owned()
                    })
                }
            }),
            secondary_dns: TextField::<Option<Ipv4Addr>>::text(|raw_value| {
                if raw_value.trim().is_empty() {
                    Ok(None)
                } else {
                    Ipv4Addr::from_str(&raw_value).map(Some).map_err(|_| {
                        "Invalid IP address format, expected XXX.XXX.XXX.XXX".to_owned()
                    })
                }
            }),
        }
    }

    fn has_errors(&self) -> bool {
        self.ssid.has_errors()
            || self.auth.has_errors()
            || self.auth.value() != Some(AuthMethod::None)
                && (self.password.has_errors() || self.password_confirm.has_errors())
            || self.ip_conf_enabled.has_errors()
            || self.ip_conf_enabled.value() == Some(true)
                && (self.dhcp_enabled.has_errors()
                    || self.dhcp_enabled.value() != Some(true)
                        && (self.subnet.has_errors()
                            || self.ip.has_errors()
                            || self.dns.has_errors()
                            || self.secondary_dns.has_errors()))
    }

    fn get(&self) -> Option<ClientConfiguration> {
        if self.has_errors() {
            None
        } else {
            Some(ClientConfiguration {
                ssid: self.ssid.value().unwrap(),

                auth_method: self.auth.value().unwrap(),
                password: self.password.value().unwrap_or_else(|| String::new()),

                ip_conf: if self.ip_conf_enabled.value().unwrap() {
                    Some(if self.dhcp_enabled.value().unwrap() {
                        ipv4::ClientConfiguration::DHCP(DHCPClientSettings { hostname: None })
                    } else {
                        ipv4::ClientConfiguration::Fixed(ipv4::ClientSettings {
                            subnet: self.subnet.value().unwrap(),
                            ip: self.ip.value().unwrap(),
                            dns: self.dns.value().unwrap(),
                            secondary_dns: self.secondary_dns.value().unwrap(),
                        })
                    })
                } else {
                    None
                },

                ..Default::default()
            })
        }
    }

    fn set(&self, conf: &ClientConfiguration) {
        self.ssid.set(conf.ssid.clone());

        self.auth.set(conf.auth_method.to_string());
        self.password.set(conf.password.clone());
        self.password_confirm.set(conf.password.clone());

        self.ip_conf_enabled.set(conf.ip_conf.is_some());

        self.dhcp_enabled.set(
            conf.ip_conf
                .as_ref()
                .map(|i| matches!(i, ipv4::ClientConfiguration::DHCP(_)))
                .unwrap_or(false),
        );
        self.subnet.set(
            conf.as_ip_conf_ref()
                .and_then(|i| i.as_fixed_settings_ref().map(|i| i.subnet.to_string()))
                .unwrap_or_else(|| String::new()),
        );
        self.ip.set(
            conf.as_ip_conf_ref()
                .and_then(|i| i.as_fixed_settings_ref().map(|i| i.ip.to_string()))
                .unwrap_or_else(|| String::new()),
        );
        self.dns.set(
            conf.as_ip_conf_ref()
                .and_then(|i| {
                    i.as_fixed_settings_ref()
                        .and_then(|i| i.dns.map(|d| d.to_string()))
                })
                .unwrap_or_else(|| String::new()),
        );
        self.secondary_dns.set(
            conf.as_ip_conf_ref()
                .and_then(|i| {
                    i.as_fixed_settings_ref()
                        .and_then(|i| i.secondary_dns.map(|d| d.to_string()))
                })
                .unwrap_or_else(|| String::new()),
        );
    }

    fn render(&self, disabled: bool) -> Html {
        let disabled_ip = disabled || !self.ip_conf_enabled.value().unwrap_or(false);

        let hidden = if_true(disabled, "visibility: hidden;");
        let hidden_ip = if_true(disabled_ip, "visibility: hidden;");

        let input_class = |errors| classes!("input", if_true(!disabled && errors, "is-danger"));
        let input_class_ip =
            |errors| classes!("input", if_true(!disabled_ip && errors, "is-danger"));

        html! {
            <>
            // SSID
            <div class="field">
                <label class="label">{ "SSID" }</label>
                <div class="control">
                    <input
                        class={input_class(self.ssid.has_errors())}
                        type="text"
                        placeholder="0..24 characters"
                        value={self.ssid.raw_value()}
                        {disabled}
                        oninput={self.ssid.change()}
                        />
                </div>
                <p class="help is-danger" style={hidden}>{self.ssid.error_str()}</p>
            </div>

            // Authentication
            <div class="field">
                <label class="label">{"Authentication"}</label>
                <div class="control">
                    <div class="select">
                        <select disabled={disabled} onchange={self.auth.change()}>
                        {
                            AuthMethod::iter().map(|item| {
                                html! {
                                    <option value={item.to_string()} selected={Some(item) == self.auth.value()}>
                                        {item.get_message().map(str::to_owned).unwrap_or_else(|| item.to_string())}
                                    </option>
                                }
                            })
                            .collect::<Html>()
                        }
                        </select>
                    </div>
                </div>
            </div>

            {
                if self.auth.value() != Some(AuthMethod::None) {
                    html! {
                        <>
                        // Password
                        <div class="field">
                            <label class="label">{if self.auth.value() == Some(AuthMethod::WEP) { "Key" } else { "Password" }}</label>
                            <div class="control">
                                <input
                                    class={input_class(self.password.has_errors())}
                                    type="password"
                                    placeholder="0..24 characters"
                                    value={self.password.raw_value()}
                                    disabled={disabled}
                                    oninput={self.password.change()}
                                    />
                            </div>
                            <p class="help is-danger" style={hidden}>{self.password.error_str()}</p>
                        </div>

                        // Confirm password
                        <div class="field">
                            <label class="label">{if self.auth.value() == Some(AuthMethod::WEP) { "Key Confirmation" } else { "Password Confirmation" }}</label>
                            <div class="control">
                                <input
                                    class={input_class(self.password_confirm.has_errors())}
                                    type="password"
                                    placeholder="0..24 characters"
                                    value={self.password_confirm.raw_value()}
                                    disabled={disabled}
                                    oninput={self.password_confirm.change()}
                                    />
                            </div>
                            <p class="help is-danger" style={hidden}>{self.password_confirm.error_str()}</p>
                        </div>
                        </>
                    }
                } else {
                    html! {}
                }
            }

            // IP Configuration
            <div class="field">
                <label class="checkbox" {disabled}>
                    <input
                        type="checkbox"
                        checked={self.ip_conf_enabled.raw_value()}
                        {disabled}
                        onclick={self.ip_conf_enabled.change()}
                    />
                    {"IP Configuration"}
                </label>
            </div>

            // DHCP
            <div class="field">
                <label class="checkbox" disabled={disabled_ip}>
                    <input
                        type="checkbox"
                        checked={self.dhcp_enabled.raw_value()}
                        disabled={disabled_ip}
                        onclick={self.dhcp_enabled.change()}
                    />
                    {"DHCP"}
                </label>
            </div>

            {
                if !self.dhcp_enabled.value().unwrap_or(true) {
                    html! {
                        <>
                        // Gateway/Subnet
                        <div class="field">
                            <label class="label">{ "Gateway/Subnet" }</label>
                            <div class="control">
                                <input
                                    class={input_class_ip(self.subnet.has_errors())}
                                    type="text"
                                    placeholder="XXX.XXX.XXX.XXX/YY"
                                    value={self.subnet.raw_value()}
                                    disabled={disabled_ip}
                                    oninput={self.subnet.change()}
                                    />
                            </div>
                            <p class="help is-danger" style={hidden_ip}>{self.subnet.error_str()}</p>
                        </div>

                        // IP
                        <div class="field">
                            <label class="label">{ "IP" }</label>
                            <div class="control">
                                <input
                                    class={input_class_ip(self.ip.has_errors())}
                                    type="text"
                                    placeholder="XXX.XXX.XXX.XXX"
                                    value={self.ip.raw_value()}
                                    disabled={disabled_ip}
                                    oninput={self.ip.change()}
                                    />
                            </div>
                            <p class="help is-danger" style={hidden_ip}>{self.ip.error_str()}</p>
                        </div>

                        // DNS
                        <div class="field">
                            <label class="label">{ "DNS" }</label>
                            <div class="control">
                                <input
                                    class={input_class_ip(self.dns.has_errors())}
                                    type="text"
                                    placeholder="XXX.XXX.XXX.XXX (optional)"
                                    value={self.dns.raw_value()}
                                    disabled={disabled_ip}
                                    oninput={self.dns.change()}
                                    />
                            </div>
                            <p class="help is-danger" style={hidden_ip}>{self.dns.error_str()}</p>
                        </div>

                        // Secondary DNS
                        <div class="field">
                            <label class="label">{ "Secondary DNS" }</label>
                            <div class="control">
                                <input
                                    class={input_class_ip(self.secondary_dns.has_errors())}
                                    type="text"
                                    placeholder="XXX.XXX.XXX.XXX (optional)"
                                    value={self.secondary_dns.raw_value()}
                                    disabled={disabled_ip}
                                    oninput={self.secondary_dns.change()}
                                    />
                            </div>
                            <p class="help is-danger" style={hidden_ip}>{self.secondary_dns.error_str()}</p>
                        </div>
                        </>
                    }
                } else {
                    html! {}
                }
            }
            </>
        }
    }
}
