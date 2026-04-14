import { faker } from '@faker-js/faker';
import type { Variable, ScopeLevel } from '../types/index.js';

export interface VariableScope {
  get(key: string): string | undefined;
  set(key: string, value: string): void;
  unset(key: string): void;
  getAll(): Variable[];
  toRecord(): Record<string, string>;
}

class MapScope implements VariableScope {
  private vars: Map<string, Variable>;

  constructor(variables: Variable[] = []) {
    this.vars = new Map(
      variables.filter((v) => v.enabled).map((v) => [v.key, v]),
    );
  }

  get(key: string): string | undefined {
    return this.vars.get(key)?.value;
  }

  set(key: string, value: string): void {
    const existing = this.vars.get(key);
    if (existing) {
      existing.value = value;
    } else {
      this.vars.set(key, { key, value, type: 'string', enabled: true });
    }
  }

  unset(key: string): void {
    this.vars.delete(key);
  }

  getAll(): Variable[] {
    return [...this.vars.values()];
  }

  toRecord(): Record<string, string> {
    const record: Record<string, string> = {};
    for (const [key, v] of this.vars) {
      record[key] = v.value;
    }
    return record;
  }
}

const SCOPE_ORDER: ScopeLevel[] = ['local', 'environment', 'collection', 'global'];

function randomImageUrl(category?: string): string {
  const w = 640;
  const h = 480;
  return category
    ? `https://loremflickr.com/${w}/${h}/${category}`
    : `https://loremflickr.com/${w}/${h}`;
}

const DYNAMIC_GENERATORS: Record<string, () => string> = {
  // ── Common ──
  '$guid': () => crypto.randomUUID(),
  '$timestamp': () => String(Math.floor(Date.now() / 1000)),
  '$isoTimestamp': () => new Date().toISOString(),
  '$randomUUID': () => crypto.randomUUID(),

  // ── Text, Numbers, Colors ──
  '$randomAlphaNumeric': () => faker.string.alphanumeric(1),
  '$randomBoolean': () => String(faker.datatype.boolean()),
  '$randomInt': () => String(faker.number.int({ min: 0, max: 1000 })),
  '$randomColor': () => faker.color.human(),
  '$randomHexColor': () => faker.color.rgb(),
  '$randomAbbreviation': () => faker.hacker.abbreviation(),

  // ── Internet & IP ──
  '$randomIP': () => faker.internet.ipv4(),
  '$randomIPV6': () => faker.internet.ipv6(),
  '$randomMACAddress': () => faker.internet.mac(),
  '$randomPassword': () => faker.internet.password({ length: 15 }),
  '$randomLocale': () => faker.helpers.arrayElement(['en', 'es', 'fr', 'de', 'ja', 'ko', 'zh', 'pt', 'it', 'ru', 'nl', 'sv', 'da', 'fi', 'nb', 'pl']),
  '$randomUserAgent': () => faker.internet.userAgent(),
  '$randomProtocol': () => faker.helpers.arrayElement(['http', 'https']),
  '$randomSemver': () => faker.system.semver(),

  // ── Names ──
  '$randomFirstName': () => faker.person.firstName(),
  '$randomLastName': () => faker.person.lastName(),
  '$randomFullName': () => faker.person.fullName(),
  '$randomNamePrefix': () => faker.person.prefix(),
  '$randomNameSuffix': () => faker.person.suffix(),

  // ── Profession ──
  '$randomJobArea': () => faker.person.jobArea(),
  '$randomJobDescriptor': () => faker.person.jobDescriptor(),
  '$randomJobTitle': () => faker.person.jobTitle(),
  '$randomJobType': () => faker.person.jobType(),

  // ── Phone, Address, Location ──
  '$randomPhoneNumber': () => faker.phone.number(),
  '$randomPhoneNumberExt': () => faker.phone.number() + ' x' + faker.string.numeric(4),
  '$randomCity': () => faker.location.city(),
  '$randomStreetName': () => faker.location.street(),
  '$randomStreetAddress': () => faker.location.streetAddress(),
  '$randomCountry': () => faker.location.country(),
  '$randomCountryCode': () => faker.location.countryCode(),
  '$randomLatitude': () => String(faker.location.latitude()),
  '$randomLongitude': () => String(faker.location.longitude()),

  // ── Images ──
  '$randomAvatarImage': () => faker.image.avatar(),
  '$randomImageUrl': () => randomImageUrl(),
  '$randomAbstractImage': () => randomImageUrl('abstract'),
  '$randomAnimalsImage': () => randomImageUrl('animals'),
  '$randomBusinessImage': () => randomImageUrl('business'),
  '$randomCatsImage': () => randomImageUrl('cats'),
  '$randomCityImage': () => randomImageUrl('city'),
  '$randomFoodImage': () => randomImageUrl('food'),
  '$randomNightlifeImage': () => randomImageUrl('nightlife'),
  '$randomFashionImage': () => randomImageUrl('fashion'),
  '$randomPeopleImage': () => randomImageUrl('people'),
  '$randomNatureImage': () => randomImageUrl('nature'),
  '$randomSportsImage': () => randomImageUrl('sports'),
  '$randomTransportImage': () => randomImageUrl('transport'),
  '$randomImageDataUri': () => faker.image.dataUri(),

  // ── Finance ──
  '$randomBankAccount': () => faker.finance.accountNumber(8),
  '$randomBankAccountName': () => faker.finance.accountName(),
  '$randomCreditCardMask': () => faker.finance.creditCardNumber().slice(-4),
  '$randomBankAccountBic': () => faker.finance.bic(),
  '$randomBankAccountIban': () => faker.finance.iban(),
  '$randomTransactionType': () => faker.finance.transactionType(),
  '$randomCurrencyCode': () => faker.finance.currencyCode(),
  '$randomCurrencyName': () => faker.finance.currencyName(),
  '$randomCurrencySymbol': () => faker.finance.currencySymbol(),
  '$randomBitcoin': () => faker.finance.bitcoinAddress(),

  // ── Business ──
  '$randomCompanyName': () => faker.company.name(),
  '$randomCompanySuffix': () => faker.helpers.arrayElement(['Inc', 'LLC', 'Group', 'and Sons', 'Corp']),
  '$randomBs': () => faker.company.buzzPhrase(),
  '$randomBsAdjective': () => faker.company.buzzAdjective(),
  '$randomBsBuzz': () => faker.company.buzzVerb(),
  '$randomBsNoun': () => faker.company.buzzNoun(),
  '$randomCatchPhrase': () => faker.company.catchPhrase(),
  '$randomCatchPhraseAdjective': () => faker.company.catchPhraseAdjective(),
  '$randomCatchPhraseDescriptor': () => faker.company.catchPhraseDescriptor(),
  '$randomCatchPhraseNoun': () => faker.company.catchPhraseNoun(),

  // ── Databases ──
  '$randomDatabaseColumn': () => faker.database.column(),
  '$randomDatabaseType': () => faker.database.type(),
  '$randomDatabaseCollation': () => faker.database.collation(),
  '$randomDatabaseEngine': () => faker.database.engine(),

  // ── Dates ──
  '$randomDateFuture': () => faker.date.future().toString(),
  '$randomDatePast': () => faker.date.past().toString(),
  '$randomDateRecent': () => faker.date.recent().toString(),
  '$randomWeekday': () => faker.date.weekday(),
  '$randomMonth': () => faker.date.month(),

  // ── Domains, Emails, Usernames ──
  '$randomDomainName': () => faker.internet.domainName(),
  '$randomDomainSuffix': () => faker.internet.domainSuffix(),
  '$randomDomainWord': () => faker.internet.domainWord(),
  '$randomEmail': () => faker.internet.email(),
  '$randomExampleEmail': () => faker.internet.exampleEmail(),
  '$randomUserName': () => faker.internet.username(),
  '$randomUrl': () => faker.internet.url(),

  // ── Files & Directories ──
  '$randomFileName': () => faker.system.fileName(),
  '$randomFileType': () => faker.system.fileType(),
  '$randomFileExt': () => faker.system.fileExt(),
  '$randomCommonFileName': () => faker.system.commonFileName(),
  '$randomCommonFileType': () => faker.system.commonFileType(),
  '$randomCommonFileExt': () => faker.system.commonFileExt(),
  '$randomFilePath': () => faker.system.filePath(),
  '$randomDirectoryPath': () => faker.system.directoryPath(),
  '$randomMimeType': () => faker.system.mimeType(),

  // ── Stores / Commerce ──
  '$randomPrice': () => faker.commerce.price({ min: 0, max: 1000, dec: 2 }),
  '$randomProduct': () => faker.commerce.product(),
  '$randomProductAdjective': () => faker.commerce.productAdjective(),
  '$randomProductMaterial': () => faker.commerce.productMaterial(),
  '$randomProductName': () => faker.commerce.productName(),
  '$randomDepartment': () => faker.commerce.department(),

  // ── Grammar ──
  '$randomNoun': () => faker.hacker.noun(),
  '$randomVerb': () => faker.hacker.verb(),
  '$randomIngverb': () => faker.hacker.ingverb(),
  '$randomAdjective': () => faker.hacker.adjective(),
  '$randomWord': () => faker.lorem.word(),
  '$randomWords': () => faker.lorem.words(5),
  '$randomPhrase': () => faker.hacker.phrase(),

  // ── Lorem Ipsum ──
  '$randomLoremWord': () => faker.lorem.word(),
  '$randomLoremWords': () => faker.lorem.words(),
  '$randomLoremSentence': () => faker.lorem.sentence(),
  '$randomLoremSentences': () => faker.lorem.sentences(),
  '$randomLoremParagraph': () => faker.lorem.paragraph(),
  '$randomLoremParagraphs': () => faker.lorem.paragraphs(),
  '$randomLoremText': () => faker.lorem.text(),
  '$randomLoremSlug': () => faker.lorem.slug(),
  '$randomLoremLines': () => faker.lorem.lines(),
};

export class VariableStore {
  private scopes: Map<ScopeLevel, VariableScope> = new Map();

  constructor(config: {
    global?: Variable[];
    collection?: Variable[];
    environment?: Variable[];
  } = {}) {
    this.scopes.set('global', new MapScope(config.global));
    this.scopes.set('collection', new MapScope(config.collection));
    this.scopes.set('environment', new MapScope(config.environment));
    this.scopes.set('local', new MapScope());
  }

  getScope(level: ScopeLevel): VariableScope {
    return this.scopes.get(level)!;
  }

  resolve(template: string): string {
    return template.replace(/\{\{(.+?)\}\}/g, (match, key: string) => {
      const trimmed = key.trim();

      if (trimmed.startsWith('$')) {
        return this.resolveDynamic(trimmed);
      }

      for (const level of SCOPE_ORDER) {
        const scope = this.scopes.get(level);
        const val = scope?.get(trimmed);
        if (val !== undefined) return val;
      }

      return match;
    });
  }

  resolveHeaders(headers: Record<string, string>): Record<string, string> {
    const resolved: Record<string, string> = {};
    for (const [key, value] of Object.entries(headers)) {
      resolved[this.resolve(key)] = this.resolve(value);
    }
    return resolved;
  }

  setLocalScope(data: Record<string, string>): void {
    const scope = this.scopes.get('local')!;
    for (const [key, value] of Object.entries(data)) {
      scope.set(key, value);
    }
  }

  clearLocalScope(): void {
    this.scopes.set('local', new MapScope());
  }

  private resolveDynamic(key: string): string {
    const gen = DYNAMIC_GENERATORS[key];
    if (gen) return gen();
    return `{{${key}}}`;
  }
}
