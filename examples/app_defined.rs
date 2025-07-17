use cli_table::WithTitle;
use kcheck::config::KcheckConfigBuilder;
use kcheck::kconfig::{KconfigOption, KconfigState};
use kcheck::kernel::KernelConfigBuilder;
use kcheck::Kcheck;

fn main() {
    // Kernel configs required by this application.
    let required_kernel_configs = vec![
        KconfigOption::new("CONFIG_FOO", KconfigState::On),
        KconfigOption::new("CONFIG_BAR", KconfigState::Module),
        KconfigOption::new("CONFIG_BAZ", KconfigState::Off),
        KconfigOption::new("CONFIG_USB_ACM", KconfigState::Enabled),
    ];

    let config = KcheckConfigBuilder::default()
        .kernel(required_kernel_configs)
        .build()
        .unwrap();

    let kernel = KernelConfigBuilder::default().system().build().unwrap();

    let kcheck = Kcheck::new(config, kernel);
    let table = kcheck
        .perform_check()
        .unwrap()
        .with_title()
        .display()
        .unwrap();
    println!("{table}");
}
