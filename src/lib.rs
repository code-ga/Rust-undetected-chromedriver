use rand::Rng;
use std::error::Error;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::Duration;
// use thirtyfour::{prelude::ElementWaitable, By};
pub use thirtyfour::error::WebDriverError;
pub use thirtyfour::prelude::*;
// use thirtyfour::{DesiredCapabilities, WebDriver};

#[async_trait::async_trait]
pub trait Chrome {
    async fn new() -> Self;
    async fn bypass_cloudflare(&self, url: &str) -> Result<(), Box<dyn Error>>;
    async fn borrow(&self) -> &WebDriver;
    async fn goto(&self, url: &str) -> Result<(), Box<dyn Error>>;
}

#[async_trait::async_trait]
impl Chrome for WebDriver {
    async fn new() -> WebDriver {
        chrome().await.unwrap()
    }

    async fn goto(&self, url: &str) -> Result<(), Box<dyn Error>> {
        let driver = self.borrow().await;
        driver
            .execute(&format!(r#"window.open("{}", "_blank");"#, url), vec![])
            .await?;

        tokio::time::sleep(Duration::from_secs(3)).await;

        let first_window = driver
            .windows()
            .await?
            .first()
            .expect("Unable to get first windows")
            .clone();

        driver.switch_to_window(first_window).await?;
        driver.close_window().await?;
        let first_window = driver
            .windows()
            .await?
            .last()
            .expect("Unable to get last windows")
            .clone();
        driver.switch_to_window(first_window).await?;
        Ok(())
    }

    async fn bypass_cloudflare(&self, url: &str) -> Result<(), Box<dyn Error>> {
        let driver = self.borrow().await;
        driver.goto(url).await?;

        driver.enter_frame(0).await?;

        let button = driver.find(By::Css("#challenge-stage")).await?;

        button.wait_until().clickable().await?;

        tokio::time::sleep(Duration::from_secs(2)).await;

        button.click().await?;
        Ok(())
    }

    async fn borrow(&self) -> &WebDriver {
        self
    }
}

/// Fetches a new ChromeDriver executable and patches it to prevent detection.
/// Returns a WebDriver instance.
pub async fn chrome() -> Result<WebDriver, Box<dyn std::error::Error + Send + Sync>> {
    let os = std::env::consts::OS;
    let base_dir = match os {
        "linux" => String::from(format!(
            "{}/.undetected-chromedriver",
            match home::home_dir() {
                Some(path) => path.display().to_string(),
                None =>
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Unable to get home directory"
                    ))),
            }
        )),
        "macos" => String::from(format!(
            "{}/.undetected-chromedriver",
            match home::home_dir() {
                Some(path) => path.display().to_string(),
                None =>
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Unable to get home directory"
                    ))),
            }
        )),
        "windows" => String::from(format!(
            "{}\\AppData\\Roaming\\undetected-chromedriver",
            match home::home_dir() {
                Some(path) => path.display().to_string(),
                None =>
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Unable to get home directory"
                    ))),
            }
        )),
        _ => panic!("Unsupported OS!"),
    };
    if std::path::Path::new(&base_dir).exists()
        && std::path::Path::new(&base_dir)
            .join(match os {
                "linux" | "macos" => "./chromedriver",
                "windows" => "./chromedriver.exe",
                _ => panic!("Unsupported OS!"),
            })
            .exists()
    {
        println!("ChromeDriver already exists!");
    } else {
        println!("ChromeDriver does not exist! Fetching...");
        let client = reqwest::Client::new();
        fetch_chromedriver(&client, &base_dir).await?;
    }
    let chromedriver_executable = std::path::Path::new(&base_dir).join(match os {
        "linux" => "chromedriver_PATCHED",
        "macos" => "chromedriver_PATCHED",
        "windows" => "chromedriver_PATCHED.exe",
        _ => panic!("Unsupported OS!"),
    });
    println!(
        "ChromeDriver executable: {}",
        chromedriver_executable.clone().display()
    );
    match !chromedriver_executable.clone().exists() {
        true => {
            println!("Starting ChromeDriver executable patch...");
            let file_name = std::path::Path::new(&base_dir).join(if cfg!(windows) {
                "chromedriver.exe"
            } else {
                "chromedriver"
            });
            let f = std::fs::read(file_name)?;
            let mut new_chromedriver_bytes = f.clone();
            let mut total_cdc = String::from("");
            let mut cdc_pos_list = Vec::new();
            let mut is_cdc_present = false;
            let mut patch_ct = 0;
            for i in 0..f.len() - 3 {
                if "cdc_"
                    == format!(
                        "{}{}{}{}",
                        f[i] as char,
                        f[i + 1] as char,
                        f[i + 2] as char,
                        f[i + 3] as char
                    )
                    .as_str()
                {
                    for x in i + 4..i + 22 {
                        total_cdc.push_str(&(f[x] as char).to_string());
                    }
                    is_cdc_present = true;
                    cdc_pos_list.push(i);
                    total_cdc = String::from("");
                }
            }
            match is_cdc_present {
                true => println!("Found cdcs!"),
                false => println!("No cdcs were found!"),
            }
            let get_random_char = || -> char {
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
                    .chars()
                    .collect::<Vec<char>>()[rand::thread_rng().gen_range(0..48)]
            };

            for i in cdc_pos_list {
                for x in i + 4..i + 22 {
                    new_chromedriver_bytes[x] = get_random_char() as u8;
                }
                patch_ct += 1;
            }
            println!("Patched {} cdcs!", patch_ct);

            println!("Starting to write to binary file...");
            let _file = std::fs::File::create(chromedriver_executable.clone())?;
            match std::fs::write(chromedriver_executable.clone(), new_chromedriver_bytes) {
                Ok(_res) => {
                    println!("Successfully wrote patched executable to 'chromedriver_PATCHED'!",)
                }
                Err(err) => println!("Error when writing patch to file! Error: {}", err),
            };
        }
        false => {
            println!("Detected patched chromedriver executable!");
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let mut perms = std::fs::metadata(chromedriver_executable.clone())?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(chromedriver_executable.clone(), perms)?;
    }
    println!("Starting chromedriver...");
    let port: usize = rand::thread_rng().gen_range(2000..5000);
    Command::new(chromedriver_executable.clone())
        .arg(format!("--port={}", port))
        .spawn()
        .expect("Failed to start chromedriver!");
    let mut caps = DesiredCapabilities::chrome();
    caps.set_no_sandbox()?;
    caps.set_disable_dev_shm_usage()?;
    caps.add_chrome_arg("--disable-blink-features=AutomationControlled")?;
    caps.add_chrome_arg("window-size=1920,1080")?;
    caps.add_chrome_arg("user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36")?;
    caps.add_chrome_arg("disable-infobars")?;
    caps.add_chrome_option("excludeSwitches", ["enable-automation"])?;
    let mut driver = None;
    let mut attempt = 0;
    while driver.is_none() && attempt < 20 {
        attempt += 1;
        match WebDriver::new(&format!("http://localhost:{}", port), caps.clone()).await {
            Ok(d) => driver = Some(d),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(250)).await,
        }
    }
    let driver = match driver {
        Some(d) => d,
        None => {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Unable to start chromedriver!",
            )))
        }
    };
    Ok(driver)
}

async fn fetch_chromedriver(
    client: &reqwest::Client,
    base_dir: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let os = std::env::consts::OS;

    let installed_version = get_chrome_version(os).await?;
    let chromedriver_url: String;
    if installed_version.as_str() >= "114" {
        println!("Chrome version >= 114 detected!");
        // Fetch the correct version
        let url = "https://googlechromelabs.github.io/chrome-for-testing/latest-versions-per-milestone.json";
        let resp = client.get(url).send().await?;
        let body = resp.bytes().await?;
        let json = serde_json::from_slice::<serde_json::Value>(&body)?;
        let version = match json["milestones"][installed_version]["version"].as_str() {
            Some(v) => v,
            None => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Unable to get chromedriver version",
                )))
            }
        };

        // Fetch the chromedriver binary
        chromedriver_url = match os {
            "linux" => format!(
                "https://storage.googleapis.com/chrome-for-testing-public/{}/{}/{}",
                version, "linux64", "chromedriver-linux64.zip"
            ),
            "macos" => format!(
                "https://storage.googleapis.com/chrome-for-testing-public/{}/{}/{}",
                version, "mac-x64", "chromedriver-mac-x64.zip"
            ),
            "windows" => format!(
                "https://storage.googleapis.com/chrome-for-testing-public/{}/{}/{}",
                version, "win64", "chromedriver-win64.zip"
            ),
            _ => panic!("Unsupported OS!"),
        };
    } else {
        let resp = client
            .get(format!(
                "https://chromedriver.storage.googleapis.com/LATEST_RELEASE_{}",
                installed_version
            ))
            .send()
            .await?;
        let body = resp.text().await?;
        chromedriver_url = match os {
            "linux" => format!(
                "https://chromedriver.storage.googleapis.com/{}/chromedriver_linux64.zip",
                body
            ),
            "windows" => format!(
                "https://chromedriver.storage.googleapis.com/{}/chromedriver_win32.zip",
                body
            ),
            "macos" => format!(
                "https://chromedriver.storage.googleapis.com/{}/chromedriver_mac64.zip",
                body
            ),
            _ => panic!("Unsupported OS!"),
        };
    }

    let resp = client.get(&chromedriver_url).send().await?;
    let body = resp.bytes().await?;
    println!("Downloading chromedriver...");
    dbg!(&base_dir);
    let base_dir = std::path::Path::new(base_dir);
    if !base_dir.exists() {
        std::fs::create_dir_all(base_dir)?;
    }
    println!("{}", body.len());
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(body))?;
    println!("Extracting chromedriver...");
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = std::path::Path::new(&base_dir).join(
            match std::path::Path::new(file.name()).file_name() {
                Some(f) => f,
                None => {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Unable to get chromedriver file name",
                    )))
                }
            },
        );
        outpath.parent().map(std::fs::create_dir_all);
        println!("extracting {} to {:?}", file.name(), outpath);
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            let mut outfile = std::fs::File::create(outpath.clone())?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}

async fn get_chrome_version(os: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    println!("Getting installed Chrome version...");
    let command = match os {
        "linux" => Command::new("/usr/bin/google-chrome")
            .arg("--version")
            .output()?,
        "macos" => Command::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
            .arg("--version")
            .output()?,
        "windows" => Command::new("powershell")
            .arg("-c")
            .arg("(Get-Item 'C:/Program Files/Google/Chrome/Application/chrome.exe').VersionInfo")
            .output()?,
        _ => panic!("Unsupported OS!"),
    };
    let output = String::from_utf8(command.stdout)?;

    let version = output
        .lines()
        .flat_map(|line| line.chars().filter(|&ch| ch.is_ascii_digit()))
        .take(3)
        .collect::<String>();

    println!("Currently installed Chrome version: {}", version);
    Ok(version)
}
