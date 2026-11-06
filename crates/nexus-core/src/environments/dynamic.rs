//! The `{{$name}}` dynamic variables. Same names as the TypeScript `DYNAMIC_GENERATORS` table;
//! values follow the same shape/format but are produced by the `fake` crate plus the small
//! tables below, so the exact output differs from faker.js.

use chrono::{Duration, Local, SecondsFormat, Utc};
use fake::faker::{
    address, company, creditcard, currency, filesystem, finance, internet, job, lorem, name,
    phone_number,
};
use fake::rand::{self, seq::IndexedRandom, RngExt};
use fake::Fake;

/// Generates the value for a dynamic key (including the leading `$`), or `None` if unknown.
pub fn generate(key: &str) -> Option<String> {
    Some(match key {
        // -- Common --
        "$guid" | "$randomUUID" => uuid::Uuid::new_v4().to_string(),
        "$timestamp" => Utc::now().timestamp().to_string(),
        "$isoTimestamp" => Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),

        // -- Text, Numbers, Colors --
        "$randomAlphaNumeric" => alphanumeric(1),
        "$randomBoolean" => rand::rng().random_bool(0.5).to_string(),
        "$randomInt" => rand::rng().random_range(0..=1000).to_string(),
        "$randomColor" => pick(COLORS),
        "$randomHexColor" => format!("#{:06x}", rand::rng().random_range(0..=0xff_ffff_u32)),
        "$randomAbbreviation" => pick(ABBREVIATIONS),

        // -- Internet & IP --
        "$randomIP" => internet::en::IPv4().fake(),
        "$randomIPV6" => internet::en::IPv6().fake(),
        "$randomMACAddress" => internet::en::MACAddress().fake(),
        "$randomPassword" => alphanumeric(15),
        "$randomLocale" => pick(LOCALES),
        "$randomUserAgent" => internet::en::UserAgent().fake(),
        "$randomProtocol" => pick(&["http", "https"]),
        "$randomSemver" => semver(),

        // -- Names --
        "$randomFirstName" => name::en::FirstName().fake(),
        "$randomLastName" => name::en::LastName().fake(),
        "$randomFullName" => name::en::Name().fake(),
        "$randomNamePrefix" => pick(NAME_PREFIXES),
        "$randomNameSuffix" => pick(NAME_SUFFIXES),

        // -- Profession --
        "$randomJobArea" => pick(JOB_AREAS),
        "$randomJobDescriptor" => job::en::Seniority().fake(),
        "$randomJobTitle" => job::en::Title().fake(),
        "$randomJobType" => pick(JOB_TYPES),

        // -- Phone, Address, Location --
        "$randomPhoneNumber" => phone_number::en::PhoneNumber().fake(),
        "$randomPhoneNumberExt" => format!(
            "{} x{}",
            phone_number::en::PhoneNumber().fake::<String>(),
            digits(4)
        ),
        "$randomCity" => address::en::CityName().fake(),
        "$randomStreetName" => address::en::StreetName().fake(),
        "$randomStreetAddress" => {
            format!(
                "{} {}",
                address::en::BuildingNumber().fake::<String>(),
                address::en::StreetName().fake::<String>()
            )
        }
        "$randomCountry" => address::en::CountryName().fake(),
        "$randomCountryCode" => address::en::CountryCode().fake(),
        "$randomLatitude" => format!("{:.4}", rand::rng().random_range(-90.0..=90.0_f64)),
        "$randomLongitude" => format!("{:.4}", rand::rng().random_range(-180.0..=180.0_f64)),

        // -- Images --
        "$randomAvatarImage" => {
            format!(
                "https://avatars.githubusercontent.com/u/{}",
                rand::rng().random_range(1..=99_999_999_u32)
            )
        }
        "$randomImageUrl" => image_url(None),
        "$randomAbstractImage" => image_url(Some("abstract")),
        "$randomAnimalsImage" => image_url(Some("animals")),
        "$randomBusinessImage" => image_url(Some("business")),
        "$randomCatsImage" => image_url(Some("cats")),
        "$randomCityImage" => image_url(Some("city")),
        "$randomFoodImage" => image_url(Some("food")),
        "$randomNightlifeImage" => image_url(Some("nightlife")),
        "$randomFashionImage" => image_url(Some("fashion")),
        "$randomPeopleImage" => image_url(Some("people")),
        "$randomNatureImage" => image_url(Some("nature")),
        "$randomSportsImage" => image_url(Some("sports")),
        "$randomTransportImage" => image_url(Some("transport")),
        "$randomImageDataUri" => image_data_uri(),

        // -- Finance --
        "$randomBankAccount" => digits(8),
        "$randomBankAccountName" => pick(ACCOUNT_NAMES),
        "$randomCreditCardMask" => {
            let number: String = creditcard::en::CreditCardNumber().fake();
            let digits: Vec<char> = number.chars().filter(char::is_ascii_digit).collect();
            digits[digits.len().saturating_sub(4)..].iter().collect()
        }
        "$randomBankAccountBic" => finance::en::Bic().fake(),
        "$randomBankAccountIban" => iban(),
        "$randomTransactionType" => pick(TRANSACTION_TYPES),
        "$randomCurrencyCode" => currency::en::CurrencyCode().fake(),
        "$randomCurrencyName" => currency::en::CurrencyName().fake(),
        "$randomCurrencySymbol" => currency::en::CurrencySymbol().fake(),
        "$randomBitcoin" => bitcoin_address(),

        // -- Business --
        "$randomCompanyName" => company::en::CompanyName().fake(),
        "$randomCompanySuffix" => pick(&["Inc", "LLC", "Group", "and Sons", "Corp"]),
        "$randomBs" => company::en::Bs().fake(),
        "$randomBsAdjective" => company::en::BsAdj().fake(),
        "$randomBsBuzz" => company::en::BsVerb().fake(),
        "$randomBsNoun" => company::en::BsNoun().fake(),
        "$randomCatchPhrase" => company::en::CatchPhrase().fake(),
        "$randomCatchPhraseAdjective" => company::en::Buzzword().fake(),
        "$randomCatchPhraseDescriptor" => company::en::BuzzwordMiddle().fake(),
        "$randomCatchPhraseNoun" => company::en::BuzzwordTail().fake(),

        // -- Databases --
        "$randomDatabaseColumn" => pick(DB_COLUMNS),
        "$randomDatabaseType" => pick(DB_TYPES),
        "$randomDatabaseCollation" => pick(DB_COLLATIONS),
        "$randomDatabaseEngine" => pick(DB_ENGINES),

        // -- Dates --
        "$randomDateFuture" => js_date(rand::rng().random_range(1..=365 * 86_400)),
        "$randomDatePast" => js_date(-rand::rng().random_range(1..=365 * 86_400)),
        "$randomDateRecent" => js_date(-rand::rng().random_range(1..=86_400)),
        "$randomWeekday" => pick(WEEKDAYS),
        "$randomMonth" => pick(MONTHS),

        // -- Domains, Emails, Usernames --
        "$randomDomainName" => domain_name(),
        "$randomDomainSuffix" => internet::en::DomainSuffix().fake(),
        "$randomDomainWord" => domain_word(),
        "$randomEmail" => internet::en::FreeEmail().fake(),
        "$randomExampleEmail" => {
            let first: String = name::en::FirstName().fake();
            let last: String = name::en::LastName().fake();
            let host = pick(&["example.org", "example.com", "example.net"]);
            format!("{}.{}@{}", ascii_slug(&first), ascii_slug(&last), host)
        }
        "$randomUserName" => internet::en::Username().fake(),
        "$randomUrl" => format!("{}://{}", pick(&["http", "https"]), domain_name()),

        // -- Files & Directories --
        "$randomFileName" => filesystem::en::FileName().fake(),
        "$randomFileType" => pick(MIME_TYPES)
            .split('/')
            .next()
            .unwrap_or("application")
            .to_string(),
        "$randomFileExt" => filesystem::en::FileExtension().fake(),
        "$randomCommonFileName" => format!(
            "{}.{}",
            lorem::en::Word().fake::<String>(),
            pick(COMMON_FILE_EXTS)
        ),
        "$randomCommonFileType" => pick(&["image", "application", "audio", "video", "text"]),
        "$randomCommonFileExt" => pick(COMMON_FILE_EXTS),
        "$randomFilePath" => filesystem::en::FilePath().fake(),
        "$randomDirectoryPath" => filesystem::en::DirPath().fake(),
        "$randomMimeType" => filesystem::en::MimeType().fake(),

        // -- Stores / Commerce --
        "$randomPrice" => format!("{:.2}", rand::rng().random_range(0.0..=1000.0_f64)),
        "$randomProduct" => pick(PRODUCTS),
        "$randomProductAdjective" => pick(PRODUCT_ADJECTIVES),
        "$randomProductMaterial" => pick(PRODUCT_MATERIALS),
        "$randomProductName" => format!(
            "{} {} {}",
            pick(PRODUCT_ADJECTIVES),
            pick(PRODUCT_MATERIALS),
            pick(PRODUCTS)
        ),
        "$randomDepartment" => pick(DEPARTMENTS),

        // -- Grammar --
        "$randomNoun" => pick(HACKER_NOUNS),
        "$randomVerb" => pick(HACKER_VERBS),
        "$randomIngverb" => pick(HACKER_ING_VERBS),
        "$randomAdjective" => pick(HACKER_ADJECTIVES),
        "$randomWord" | "$randomLoremWord" => lorem::en::Word().fake(),
        "$randomWords" => words(5),
        "$randomPhrase" => format!(
            "If we {} the {}, we can get to the {} {} through the {} {} {}!",
            pick(HACKER_VERBS),
            pick(HACKER_NOUNS),
            pick(ABBREVIATIONS),
            pick(HACKER_NOUNS),
            pick(HACKER_ADJECTIVES),
            pick(ABBREVIATIONS),
            pick(HACKER_NOUNS),
        ),

        // -- Lorem Ipsum --
        "$randomLoremWords" => words(3),
        "$randomLoremSentence" => lorem::en::Sentence(5..10).fake(),
        "$randomLoremSentences" => lorem::en::Sentences(2..5).fake::<Vec<String>>().join(" "),
        "$randomLoremParagraph" => lorem::en::Paragraph(3..4).fake(),
        "$randomLoremParagraphs" => lorem::en::Paragraphs(3..4).fake::<Vec<String>>().join("\n"),
        "$randomLoremText" => match rand::rng().random_range(0..5) {
            0 => words(3),
            1 => lorem::en::Sentence(5..10).fake(),
            2 => lorem::en::Sentences(2..5).fake::<Vec<String>>().join(" "),
            3 => lorem::en::Paragraph(3..4).fake(),
            _ => lorem::en::Paragraphs(3..4).fake::<Vec<String>>().join("\n"),
        },
        "$randomLoremSlug" => lorem::en::Words(3..4).fake::<Vec<String>>().join("-"),
        "$randomLoremLines" => {
            let n = rand::rng().random_range(1..=5);
            (0..n)
                .map(|_| lorem::en::Sentence(5..10).fake::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => return None,
    })
}

/// Names of every supported dynamic variable (including the leading `$`).
pub const DYNAMIC_KEYS: &[&str] = &[
    "$guid",
    "$timestamp",
    "$isoTimestamp",
    "$randomUUID",
    "$randomAlphaNumeric",
    "$randomBoolean",
    "$randomInt",
    "$randomColor",
    "$randomHexColor",
    "$randomAbbreviation",
    "$randomIP",
    "$randomIPV6",
    "$randomMACAddress",
    "$randomPassword",
    "$randomLocale",
    "$randomUserAgent",
    "$randomProtocol",
    "$randomSemver",
    "$randomFirstName",
    "$randomLastName",
    "$randomFullName",
    "$randomNamePrefix",
    "$randomNameSuffix",
    "$randomJobArea",
    "$randomJobDescriptor",
    "$randomJobTitle",
    "$randomJobType",
    "$randomPhoneNumber",
    "$randomPhoneNumberExt",
    "$randomCity",
    "$randomStreetName",
    "$randomStreetAddress",
    "$randomCountry",
    "$randomCountryCode",
    "$randomLatitude",
    "$randomLongitude",
    "$randomAvatarImage",
    "$randomImageUrl",
    "$randomAbstractImage",
    "$randomAnimalsImage",
    "$randomBusinessImage",
    "$randomCatsImage",
    "$randomCityImage",
    "$randomFoodImage",
    "$randomNightlifeImage",
    "$randomFashionImage",
    "$randomPeopleImage",
    "$randomNatureImage",
    "$randomSportsImage",
    "$randomTransportImage",
    "$randomImageDataUri",
    "$randomBankAccount",
    "$randomBankAccountName",
    "$randomCreditCardMask",
    "$randomBankAccountBic",
    "$randomBankAccountIban",
    "$randomTransactionType",
    "$randomCurrencyCode",
    "$randomCurrencyName",
    "$randomCurrencySymbol",
    "$randomBitcoin",
    "$randomCompanyName",
    "$randomCompanySuffix",
    "$randomBs",
    "$randomBsAdjective",
    "$randomBsBuzz",
    "$randomBsNoun",
    "$randomCatchPhrase",
    "$randomCatchPhraseAdjective",
    "$randomCatchPhraseDescriptor",
    "$randomCatchPhraseNoun",
    "$randomDatabaseColumn",
    "$randomDatabaseType",
    "$randomDatabaseCollation",
    "$randomDatabaseEngine",
    "$randomDateFuture",
    "$randomDatePast",
    "$randomDateRecent",
    "$randomWeekday",
    "$randomMonth",
    "$randomDomainName",
    "$randomDomainSuffix",
    "$randomDomainWord",
    "$randomEmail",
    "$randomExampleEmail",
    "$randomUserName",
    "$randomUrl",
    "$randomFileName",
    "$randomFileType",
    "$randomFileExt",
    "$randomCommonFileName",
    "$randomCommonFileType",
    "$randomCommonFileExt",
    "$randomFilePath",
    "$randomDirectoryPath",
    "$randomMimeType",
    "$randomPrice",
    "$randomProduct",
    "$randomProductAdjective",
    "$randomProductMaterial",
    "$randomProductName",
    "$randomDepartment",
    "$randomNoun",
    "$randomVerb",
    "$randomIngverb",
    "$randomAdjective",
    "$randomWord",
    "$randomWords",
    "$randomPhrase",
    "$randomLoremWord",
    "$randomLoremWords",
    "$randomLoremSentence",
    "$randomLoremSentences",
    "$randomLoremParagraph",
    "$randomLoremParagraphs",
    "$randomLoremText",
    "$randomLoremSlug",
    "$randomLoremLines",
];

fn pick(table: &[&str]) -> String {
    table
        .choose(&mut rand::rng())
        .copied()
        .unwrap_or_default()
        .to_string()
}

fn alphanumeric(len: usize) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| *CHARS.choose(&mut rng).unwrap() as char)
        .collect()
}

fn digits(len: usize) -> String {
    let mut rng = rand::rng();
    (0..len)
        .map(|_| char::from(b'0' + rng.random_range(0..10u8)))
        .collect()
}

fn words(n: usize) -> String {
    lorem::en::Words(n..n + 1).fake::<Vec<String>>().join(" ")
}

fn semver() -> String {
    let mut rng = rand::rng();
    format!(
        "{}.{}.{}",
        rng.random_range(0..=9),
        rng.random_range(0..=9),
        rng.random_range(0..=9)
    )
}

fn image_url(category: Option<&str>) -> String {
    match category {
        Some(c) => format!("https://loremflickr.com/640/480/{c}"),
        None => "https://loremflickr.com/640/480".to_string(),
    }
}

fn image_data_uri() -> String {
    let color = format!("{:06x}", rand::rng().random_range(0..=0xff_ffff_u32));
    format!(
        "data:image/svg+xml;charset=UTF-8,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20version%3D%221.1%22%20baseProfile%3D%22full%22%20width%3D%22640%22%20height%3D%22480%22%3E%3Crect%20width%3D%22100%25%22%20height%3D%22100%25%22%20fill%3D%22%23{color}%22%2F%3E%3Ctext%20x%3D%22320%22%20y%3D%22240%22%20font-size%3D%2220%22%20alignment-baseline%3D%22middle%22%20text-anchor%3D%22middle%22%20fill%3D%22white%22%3E640x480%3C%2Ftext%3E%3C%2Fsvg%3E"
    )
}

/// IBAN in GB format with a valid mod-97 checksum.
fn iban() -> String {
    let mut rng = rand::rng();
    let bank: String = (0..4)
        .map(|_| char::from(b'A' + rng.random_range(0..26u8)))
        .collect();
    let rest = digits(14);
    let bban = format!("{bank}{rest}");
    // Check digits: 98 - (bban + "GB00" as digits) mod 97.
    let numeric: String = format!("{bban}GB00")
        .chars()
        .map(|c| {
            if c.is_ascii_alphabetic() {
                (c.to_ascii_uppercase() as u8 - b'A' + 10).to_string()
            } else {
                c.to_string()
            }
        })
        .collect();
    let rem = numeric
        .bytes()
        .fold(0u32, |acc, b| (acc * 10 + u32::from(b - b'0')) % 97);
    format!("GB{:02}{}", 98 - rem, bban)
}

fn bitcoin_address() -> String {
    const BASE58: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut rng = rand::rng();
    let len = rng.random_range(25..=33);
    let prefix = if rng.random_bool(0.5) { '1' } else { '3' };
    std::iter::once(prefix)
        .chain((0..len).map(|_| *BASE58.choose(&mut rng).unwrap() as char))
        .collect()
}

/// `Date.prototype.toString()`-style local time offset from now by `offset_secs`.
fn js_date(offset_secs: i64) -> String {
    let dt = Local::now() + Duration::seconds(offset_secs);
    dt.format("%a %b %d %Y %H:%M:%S GMT%z").to_string()
}

fn ascii_slug(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_lowercase()
}

fn domain_word() -> String {
    let w = ascii_slug(&name::en::LastName().fake::<String>());
    if w.is_empty() {
        "example".to_string()
    } else {
        w
    }
}

fn domain_name() -> String {
    format!(
        "{}.{}",
        domain_word(),
        internet::en::DomainSuffix().fake::<String>()
    )
}

const COLORS: &[&str] = &[
    "red",
    "green",
    "blue",
    "yellow",
    "purple",
    "mint green",
    "teal",
    "white",
    "black",
    "orange",
    "pink",
    "grey",
    "maroon",
    "violet",
    "turquoise",
    "tan",
    "sky blue",
    "salmon",
    "plum",
    "orchid",
    "olive",
    "magenta",
    "lime",
    "ivory",
    "indigo",
    "gold",
    "fuchsia",
    "cyan",
    "azure",
    "lavender",
    "silver",
];
const ABBREVIATIONS: &[&str] = &[
    "TCP", "HTTP", "SDD", "RAM", "GB", "CSS", "SSL", "AGP", "SQL", "FTP", "PCI", "AI", "ADP",
    "RSS", "XML", "EXE", "COM", "HDD", "THX", "SMTP", "SMS", "USB", "PNG", "SAS", "IB", "SCSI",
    "JSON", "XSS", "JBOD", "CLI",
];
const LOCALES: &[&str] = &[
    "en", "es", "fr", "de", "ja", "ko", "zh", "pt", "it", "ru", "nl", "sv", "da", "fi", "nb", "pl",
];
const NAME_PREFIXES: &[&str] = &["Mr.", "Mrs.", "Ms.", "Miss", "Dr."];
const NAME_SUFFIXES: &[&str] = &[
    "Jr.", "Sr.", "I", "II", "III", "IV", "V", "MD", "DDS", "PhD", "DVM",
];
const JOB_AREAS: &[&str] = &[
    "Marketing",
    "Brand",
    "Data",
    "Operations",
    "Research",
    "Accounts",
    "Security",
    "Creative",
    "Communications",
    "Program",
    "Optimization",
    "Intranet",
    "Web",
    "Directives",
    "Tactics",
    "Mobility",
    "Interactions",
    "Factors",
    "Applications",
    "Identity",
    "Metrics",
    "Usability",
    "Implementation",
    "Quality",
    "Integration",
    "Response",
    "Configuration",
    "Infrastructure",
    "Assurance",
    "Markets",
];
const JOB_TYPES: &[&str] = &[
    "Supervisor",
    "Associate",
    "Executive",
    "Liaison",
    "Officer",
    "Manager",
    "Engineer",
    "Specialist",
    "Director",
    "Coordinator",
    "Administrator",
    "Architect",
    "Analyst",
    "Designer",
    "Planner",
    "Orchestrator",
    "Technician",
    "Developer",
    "Producer",
    "Consultant",
    "Assistant",
    "Facilitator",
    "Agent",
    "Representative",
    "Strategist",
];
const ACCOUNT_NAMES: &[&str] = &[
    "Checking Account",
    "Savings Account",
    "Money Market Account",
    "Investment Account",
    "Home Loan Account",
    "Credit Card Account",
    "Auto Loan Account",
    "Personal Loan Account",
];
const TRANSACTION_TYPES: &[&str] = &["deposit", "withdrawal", "payment", "invoice"];
const DB_COLUMNS: &[&str] = &[
    "id",
    "title",
    "name",
    "email",
    "phone",
    "token",
    "group",
    "category",
    "password",
    "comment",
    "avatar",
    "status",
    "createdAt",
    "updatedAt",
];
const DB_TYPES: &[&str] = &[
    "int",
    "varchar",
    "text",
    "date",
    "datetime",
    "tinyint",
    "time",
    "timestamp",
    "smallint",
    "mediumint",
    "bigint",
    "decimal",
    "float",
    "double",
    "real",
    "bit",
    "boolean",
    "serial",
    "blob",
    "binary",
    "enum",
    "set",
    "geometry",
    "point",
];
const DB_COLLATIONS: &[&str] = &[
    "utf8_unicode_ci",
    "utf8_general_ci",
    "utf8_bin",
    "ascii_bin",
    "ascii_general_ci",
    "cp1250_bin",
    "cp1250_general_ci",
];
const DB_ENGINES: &[&str] = &["InnoDB", "MyISAM", "MEMORY", "CSV", "BLACKHOLE", "ARCHIVE"];
const WEEKDAYS: &[&str] = &[
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const MONTHS: &[&str] = &[
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const MIME_TYPES: &[&str] = &[
    "application/json",
    "application/pdf",
    "application/zip",
    "audio/mpeg",
    "audio/ogg",
    "font/woff2",
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/svg+xml",
    "text/plain",
    "text/html",
    "text/css",
    "video/mp4",
    "video/webm",
    "model/gltf+json",
    "message/rfc822",
    "multipart/form-data",
];
const COMMON_FILE_EXTS: &[&str] = &[
    "gif", "jpeg", "jpg", "png", "pdf", "mp3", "mp4", "json", "txt", "csv", "html", "zip",
];
const PRODUCTS: &[&str] = &[
    "Chair", "Car", "Computer", "Keyboard", "Mouse", "Bike", "Ball", "Gloves", "Pants", "Shirt",
    "Table", "Shoes", "Hat", "Towels", "Soap", "Tuna", "Chicken", "Fish", "Cheese", "Bacon",
    "Pizza", "Salad", "Sausages", "Chips",
];
const PRODUCT_ADJECTIVES: &[&str] = &[
    "Small",
    "Ergonomic",
    "Rustic",
    "Intelligent",
    "Gorgeous",
    "Incredible",
    "Fantastic",
    "Practical",
    "Sleek",
    "Awesome",
    "Generic",
    "Handcrafted",
    "Handmade",
    "Licensed",
    "Refined",
    "Unbranded",
    "Tasty",
];
const PRODUCT_MATERIALS: &[&str] = &[
    "Steel", "Wooden", "Concrete", "Plastic", "Cotton", "Granite", "Rubber", "Metal", "Soft",
    "Fresh", "Frozen",
];
const DEPARTMENTS: &[&str] = &[
    "Books",
    "Movies",
    "Music",
    "Games",
    "Electronics",
    "Computers",
    "Home",
    "Garden",
    "Tools",
    "Grocery",
    "Health",
    "Beauty",
    "Toys",
    "Kids",
    "Baby",
    "Clothing",
    "Shoes",
    "Jewelry",
    "Sports",
    "Outdoors",
    "Automotive",
    "Industrial",
];
const HACKER_NOUNS: &[&str] = &[
    "driver",
    "protocol",
    "bandwidth",
    "panel",
    "microchip",
    "program",
    "port",
    "card",
    "array",
    "interface",
    "system",
    "sensor",
    "firewall",
    "hard drive",
    "pixel",
    "alarm",
    "feed",
    "monitor",
    "application",
    "transmitter",
    "bus",
    "circuit",
    "capacitor",
    "matrix",
];
const HACKER_VERBS: &[&str] = &[
    "back up",
    "bypass",
    "hack",
    "override",
    "compress",
    "copy",
    "navigate",
    "index",
    "connect",
    "generate",
    "quantify",
    "calculate",
    "synthesize",
    "input",
    "transmit",
    "program",
    "reboot",
    "parse",
];
const HACKER_ING_VERBS: &[&str] = &[
    "backing up",
    "bypassing",
    "hacking",
    "overriding",
    "compressing",
    "copying",
    "navigating",
    "indexing",
    "connecting",
    "generating",
    "quantifying",
    "calculating",
    "synthesizing",
    "transmitting",
    "programming",
    "parsing",
];
const HACKER_ADJECTIVES: &[&str] = &[
    "auxiliary",
    "primary",
    "back-end",
    "digital",
    "open-source",
    "virtual",
    "cross-platform",
    "redundant",
    "online",
    "haptic",
    "multi-byte",
    "bluetooth",
    "wireless",
    "1080p",
    "neural",
    "optical",
    "solid state",
    "mobile",
];
