# kioku

[English](README.md) | **Türkçe**

[Claude Code](https://claude.com/claude-code) için kişisel bir ikinci beyin. Sen konuşursun,
notları Claude tutar.

- **Hafıza:** Claude seni, aldığın kararları ve üzerinde çalıştığın işleri oturumlar ve projeler arasında hatırlar.
- **Wiki:** verdiğin linkler ve PDF'ler birbirine bağlı ve zamanla büyüyen bir wiki'ye işlenir. Bu, Andrej Karpathy'nin [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) örüntüsüne dayanıyor.
- **Günlük:** kendi notların, yapılacakların ve fikirlerin, söylediğin anda sohbetten kaydedilir.

Her şey senin klasöründe düz Markdown dosyaları olarak durur. Klasörü
[Obsidian](https://obsidian.md) vault'u olarak açabilirsin ve git ile sürümlenir. Sunucu,
veritabanı ya da API anahtarı gerekmez; hook'lar küçük bash betikleridir.

## Nasıl çalışır

```
~/Documents/Kioku/            vault'un (özel, kendi git deposun)
  CLAUDE.md                   şemayı içe alır + kişisel kuralların
  Core.md                     sen kimsin: "beni tanı" röportajıyla dolar
  daily/                      Claude'un iş kaydı: kararlar, sonuçlar, öğrenimler, açık uçlar
  journal/                    zaman çizelgen: yaptıkların, yapılacaklar (tarihli), kaydettiklerin
  calendar/                   etkinlikler, her biri bir sayfa (CalDAV takvimine eşitlenir)
  contacts/                   kişiler ve iletişim bilgileri (CardDAV rehberine eşitlenir)
  projects/                   proje başına bir not: durum, sonraki adım, kararlar
  raw/                        işlenen kaynakların birebir kopyası (asla değiştirilmez)
  wiki/                       derlenen sayfalar: topics/ (kategori hub'ları) concepts/ entities/ ideas/ sources/ synthesis/
  index.md  log.md            beynin haritası (önce konular), yalnızca sona eklenen işlem kaydı
  .kioku → kioku/      bu repoya bağlantı: şema, hook'lar, skill'ler, CLI, şablonlar
```

Claude Code'a dört noktadan bağlanır:

- **SessionStart hook'u:** hangi projede olursa olsun her oturum yaklaşık 1k token'lık kısa bir bağlamla başlar: `Core.md`, o klasörün notu, aktif projeler, vakti gelen yapılacaklar, iş kaydının son iki günü ve bir wiki özeti. Günlüğün bu bağlama girmez; tek istisna vakti gelen yapılacaklardır.
- **Stop hook'u:** Claude'dan iki durumda, her biri için bir kez, kayıt yapması istenir:
  - oturumda gerçek bir iş yapılmış ama hiçbir şey kaydedilmemişse;
  - bir öğrenme iş günlüğüne yazılmış ama wikiye işlenmemişse.

  Kısa soru-cevap oturumlarına karışmaz.
- **Yazma kontrolü (PostToolUse):** Claude vault'a bir dosya yazar yazmaz frontmatter'ı, `Topics:` satırı, `[[bağlantılar]]` ve yapılacaklar/etkinlikler kontrol edilir; Claude hatayı aynı turda düzeltir.
- **Sıkıştırmadan önce (PreCompact):** uzun bir oturum sıkıştırılırken özet henüz kaydedilmemiş şeyleri listeler, Claude da hemen ardından onları kaydeder.
- **Her mesajdan önce (UserPromptSubmit):** autosync açıksa önce vault pull edilir, böylece başka makinede yazılan notlar görünür.
- **Skill'ler:**
  - `kioku` hafızayı kaydeder ve tanışma röportajını yürütür;
  - `kioku-journal` zaman çizelgeni ve yapılacaklarını tutar;
  - `kioku-note` araçları, siteleri, fikirleri ve öğrendiklerini konu hub'larında kategorize edilmiş, birbirine bağlı wiki sayfalarına dönüştürür;
  - `kioku-calendar` ve `kioku-contacts` etkinlikleri ve kişileri tutar;
  - `kioku-ingest` kaynak ekler;
  - `kioku-ask` kaynak göstererek cevap verir;
  - `kioku-lint` sağlık kontrolü yapar.
- **Şema ([SCHEMA.md](SCHEMA.md)):** Claude'un uyduğu kurallar: neyin nereye yazılacağı, diller, kaynak gösterme, asla yazılmayacaklar (sırlar), commit biçimi.

Şemanın ana fikri LLM Wiki'den geliyor: her soruda ham belgeleri yeniden aramak (RAG)
yerine, Claude bir kaynağı bir kez okur ve kavram ve varlık sayfalarına işler. Çelişkiler
işaretlenir ve her iddia kaynağına bağlı kalır.

## Gereksinimler

- Claude Code
- bash 4+, `jq`, `ripgrep`, `git` (Nix paketi bunları kendisi getirir)
- `node` (isteğe bağlı): Latin dışı başlıklardan dosya adı üretmek için (`машинное-обучение.md`, `機械学習.md`)
- Obsidian (isteğe bağlı): sadece gezinmek için

Linux'ta (NixOS) geliştirildi ve kullanılıyor. macOS'ta daha yeni bir bash ile çalışması beklenir (`brew install bash jq ripgrep`). Windows'ta WSL kullan. macOS ve Windows henüz test edilmedi.

## Kurulum

### NixOS / Home Manager (flake)

Flake'i input olarak ekle ve Home Manager modülünü aç:

```nix
# flake.nix
inputs.kioku = {
  url = "github:Drum3X/kioku";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

```nix
# bir Home Manager modülü
{config, inputs, ...}: {
  imports = [inputs.kioku.homeManagerModules.default];
  programs.kioku = {
    enable = true;
    vault = "${config.home.homeDirectory}/Documents/Kioku"; # varsayılan
  };
}
```

Modül şunları yapar:
- **Komut:** `kioku` komutunu ve kullandığı araçları (`jq`, `ripgrep`, `git`, `node`) PATH'e koyar.
- **Bağlantılar:** her switch'te `install.sh` ile aynı adımları çalıştırır. Vault'taki `.kioku` bağlantısı Nix store'daki pakete gider; sürüm `flake.lock`'ta sabittir.
- **Güncelleme:** `nix flake update kioku`, ardından rebuild.

`~/.claude/settings.json` ve `~/.claude/CLAUDE.md` dosyalarını Claude Code da düzenlediği için modül bu dosyaların sahibi olmaz. Kendi satırlarını, `install.sh` gibi, onlara ekler.

**Kioku'nun kendisi üzerinde çalışıyorsan:** `programs.kioku.source = "/checkout/yolu";` ayarla. Vault o zaman bu checkout'a bağlanır ve yaptığın değişiklikler rebuild gerektirmeden geçerli olur. Sabit sürüme dönmek için satırı sil.

**Kaldırma:** modülü kapatmak Claude Code bağlantılarını kaldırmaz. Önce paketteki `kioku-install --uninstall` komutunu çalıştır.

### Diğer sistemler

```bash
git clone https://github.com/Drum3X/kioku ~/Projects/kioku
```

```bash
~/Projects/kioku/install.sh
```

Klonladığın klasörü yerinde bırak, çünkü vault ona bağlanıyor. Varsayılan vault
`~/Documents/Kioku`. Başka bir klasör ya da mevcut bir Obsidian vault'u için `--vault YOL`
kullan; mevcut notlarına dokunulmaz. Kurulum betiği şunları yapar:

1. vault klasörlerini ve başlangıç dosyalarını oluşturur. Var olan dosyaları asla ezmez.
2. `vault/.kioku`'i bu klasöre bağlar; vault henüz bir git deposu değilse `git init` çalıştırır.
3. skill'leri `~/.claude/skills/` altına, `kioku` komutunu da `~/.local/bin/` altına bağlar (bu klasör `PATH`'te değilse uyarır).
4. `~/.claude/CLAUDE.md` dosyasına kısa bir Kioku bölümü ekler.
5. `~/.claude/settings.json` dosyasına hook'ları, vault için okuma/düzenleme izinlerini ve bir sandbox yazma kuralını ekler. Yedeği `settings.json.kioku.bak` olarak saklanır.

Tekrar çalıştırmak güvenli. Sonra yeni bir Claude Code oturumu aç ve **"beni tanı"** de.

## Kullanım

Normal konuş, istediğin dilde. Birkaç örnek:

| Sen dersin ki | Ne olur |
| --- | --- |
| "beni tanı" | Sen bitir diyene kadar tek tek soru sorulan açık uçlu bir röportaj; sonunda onayına sunulan bir `Core.md` taslağı |
| *(Claude ile bir işi bitirirsin)* | `daily/` ve proje notuna birer satır, ardından `Beyne kaydettim: …` |
| "bunu beyne at: https://…" | `raw/`'a kaydedilir, ana fikirler konuşulur, özet ve kavram/varlık sayfaları yazılır, index ve log güncellenir |
| "X hakkında ne biliyoruz?" | Wiki'den `[[kaynaklı]]` cevap; beyinde yoksa bunu söyler |
| "not al: bugün sınavı bitirdim" / "cuma için hatırlat: ödev teslimi" | Bugünkü `journal/` dosyasına bir satır ("cuma" gibi göreli tarihler gerçek tarihe çevrilir) |
| "yapılacaklarım ne?" / "ödevi bitirdim" | Gruplanmış liste / ilgili madde yerinde işaretlenir |
| "kaydet: Ghostty, denemek istediğim GPU hızlandırmalı terminal" / "fikir: zaman döngülü küçük bir oyun" / "şunu öğrendim: `nix why-depends` bağımlılık zincirini gösterir" | Her biri bir wiki sayfası olur (`entities/`, `ideas/`, `concepts/`) ve *Linux › Tools* gibi bir konu hub'ına yerleşir; uyan konu yoksa yeni hub açılır ve haber verilir |
| "cuma 14:00 diş randevusu" / "her pazartesi 10:00 ders" / "yarınki toplantıyı iptal et" | `calendar/` altında bir sayfa, takvimine eşitlenir |
| "Ahmet'i rehbere ekle, numarası …" | `contacts/` altında bir sayfa, rehberine eşitlenir |
| "bu hafta ne yaptım?" | `kioku recap` özeti |
| "beyni kontrol et" | Önce bir lint raporu, düzeltmeler ancak senin onayınla |
| *(terminalde ya da kısayolla)* `kioku capture "yarın bankayı ara"` | Oturum açmadan `raw/inbox/` klasörüne düşer; sonraki oturum işlemeyi önerir |

`Core.md` değişiklikleri her zaman senin onayını ister. Geri kalan her şey sormadan
kaydedilir ve her yazım vault'unda bir git commit'i olur. Claude, birden fazla makine için `kioku remote`'u açmadıysan asla push yapmaz; o sana kalmış.

CLI'ı elle de `kioku` komutuyla kullanabilirsin (`vault/.kioku/bin/kioku` dosyasına bir bağlantı):

```
kioku agent <job>|setup|status | notify "<subject>" | doctor | capture "<metin>" | context | today | journal | todo [--soon] | agenda [--days N] [--past] | new <tür> "<Başlık>" | log <işlem> "<Başlık>"
kioku search [--raw] <terim> | recap [--days N] | sync [--check|--dry-run|--full] | remote on|off|status|now | password status|set|move|remove | update [--check] | version | lint-check
```

## Hızlı yakalama ve sorun giderme

- **`kioku capture "metin ya da link"`** bir düşünceyi, yapılacağı ya da linki oturum açmadan `raw/inbox/` klasörüne atar (dosya için `kioku capture --file F`, stdin için `kioku capture -`). Commit edildiği için autosync açıksa diğer makinelere de gider. Sonraki oturum kaç tane beklediğini söyler ve işlemeyi önerir: notlar sanki sohbette söylenmiş gibi journal'a, wikiye ya da takvime gider; linkler ve dosyalar ingest edilir. Klavye kısayolu için `kioku capture --gui` küçük bir pencerede sorar (kdialog ya da zenity); Nix modülünde `programs.kioku.capture.gui = true;` bu araçları ve kısayol atanabilecek bir "Kioku capture" masaüstü girdisini getirir.
- **`kioku doctor`** Kioku'nun dayandığı her şeyi kontrol eder (araçlar, vault bağlantısı, hook'lar, skill'ler, Antigravity, git ve autosync, CalDAV/CardDAV girişi, vault sağlığı) ve her sorun için ne yapılacağını söyler. `--offline` ağ kontrollerini atlar.

## Takvim, rehber ve yapılacaklar (isteğe bağlı eşitleme)

Etkinlikler (`calendar/`), kişiler (`contacts/`) ve journal'daki yapılacaklar herhangi bir
CalDAV/CardDAV sunucusuna (SOGo/mailcow, Nextcloud, Radicale, Fastmail…) gönderilebilir. Böylece
onları Thunderbird'de, telefonunda ya da webmail'de görürsün. Eşitleme **tek yönlü**:
Claude'a söylersin, vault değişir, commit'ten sonra bir git hook'u değişikliği gönderir.

- **Neye dokunur:** seçilen takvim ve rehberde yalnızca `kioku-…` adlı kayıtlara. Diğer etkinlik ve kişilerine asla dokunulmaz.
- **Başka yerde yapılan düzenleme:** bir Kioku kaydını başka bir uygulamada düzenlersen, bir sonraki eşitleme onu vault'taki hâliyle ezer.

Kurulum:

1. Sunucun destekliyorsa hesabın için yalnızca CalDAV/CardDAV yetkili bir app password oluştur.
2. `~/.config/kioku/sync.conf` dosyasını oluştur:

   ```ini
   # sunucunun DAV kökü (/.well-known/caldav buraya yönlenir)
   url=https://mail.example.com/SOGo/dav/
   user=sen@example.com
   # koleksiyon adı ya da görünen adı
   calendar=personal
   addressbook=personal
   ```

3. Şifreyi kaydet. Bunu terminalde kendin yaz:

   ```bash
   kioku password set
   ```

   Komut şifreyi ekranda göstermeden iki kez sorar, girişi dener ve yalnızca giriş başarılıysa kaydeder:
   - **Masaüstünde:** sistemin anahtar zincirine yazar (Linux'ta KDE Wallet ya da GNOME Keyring gibi Secret Service; macOS'ta Keychain).
   - **Anahtar zinciri yoksa** (sunucu, WSL): `~/.config/kioku/dav.pass` dosyasına `chmod 600` ile yazar. Bu dosyayı başkaları okuyabiliyorsa eşitleme çalışmayı reddeder.

   Diğer komutlar:
   - `--store keyring|file` ile yeri kendin seçebilirsin.
   - `kioku password status` şifrenin nerede olduğunu gösterir; şifrenin kendisini asla göstermez.
   - `kioku password move keyring|file` şifreyi taşır.
   - `kioku password remove` şifreyi siler.

   `set`, `move` ve `remove` yalnızca gerçek bir terminalde çalışır; böylece hiçbir betik ya da ajan bu komutlara şifre veremez. Başka şifre yöneticileri de kullanılabilir: `password_cmd=` alanına şifreyi yazdıran bir komut yaz, örneğin `pass show dav` ya da `op read op://…`.

4. Bağlantıyı test et; bu komut yalnızca okur:

   ```bash
   ~/Documents/Kioku/.kioku/bin/kioku sync --check
   ```

5. Thunderbird'de ve telefonunda aynı takvime ve adres defterine abone ol. Android'de DAVx⁵ kullan (yapılacaklar için Tasks.org ya da jtx Board). iOS'ta bir CalDAV/CardDAV hesabı ekle; yapılacaklar Anımsatıcılar'da görünür.

**Hatırlatıcılar** cihazlarına bildirim olarak gelir:
- **Etkinlikler:** "yarım saat önce hatırlat" dersen etkinliğe `alarm: 30m` eklenir.
- **Saati olan yapılacaklar:** saati belli bir yapılacak iş (`(at: 17:00)`) o saatte bildirim verir.
- **Daha erken hatırlatma:** `(remind: 1h)` daha önce hatırlatır; `30m`, `1d` ya da `0` da kullanılabilir. Saat verilmemişse hatırlatma son tarihin 09:00'undan geriye doğru sayılır.

Kurulum betiği vault'a iki git hook'u ekler: `pre-commit` yeni kayıtlara kalıcı kimlik verir,
`post-commit` eşitlemeyi arka planda çalıştırır. Kayıtlar
`~/.local/state/kioku/sync.log` dosyasına yazılır. Ne gönderileceğini görmek için
`kioku sync --dry-run`, hepsini yeniden göndermek için `kioku sync --full` kullan. Eşitleme
`node` gerektirir.

## Birden fazla makine ve telefon (isteğe bağlı)

Vault'un bir kopyasını ikinci bir makinede tutabilirsin. Bu makine örneğin sürekli açık bir sunucu olabilir. Telefondan oradaki Claude ile [Remote Control](https://code.claude.com/docs/en/remote-control) üzerinden konuşursun.

1. Vault'u bir kez özel bir git reposuna push et: `git -C ~/Documents/Kioku push -u origin HEAD`.
2. Her makinede `kioku remote on` çalıştır. Bundan sonra:
   - her oturum bir `git pull` ile başlar, her mesajdan önce de yeniden pull edilir (en fazla 30 saniyede bir), böylece açık bir oturum başka makinede yazılan notları da görür;
   - her commit arka planda pull edilir, rebase yapılır ve push edilir.
3. Sunucuda:
   - bu repoyu ve vault'u klonla, `install.sh` çalıştır, ardından `kioku remote on`;
   - vault klasöründe `claude remote-control` başlat (tmux içinde ya da servis olarak);
   - sunucu, Claude uygulamasında Code bölümünde görünür.

İki makine aynı günlük ya da journal dosyasına satır eklerse git iki tarafı da korur (`.gitattributes` içindeki `merge=union`). Gerçek bir çakışmada hiçbir şey zorla birleştirilmez: yerel commit'ler kalır, push yapılmaz ve bir sonraki oturum seni uyarır.

Sunucu UTC'de çalışıyorsa `TZ` ayarla; böylece "bugün" ve etkinlik saatleri seninkiyle aynı olur. Durumu `kioku remote status` gösterir, kayıtlar `~/.local/state/kioku/git-sync.log` dosyasına yazılır.

## Zamanlanmış işler (isteğe bağlı)

Sürekli açık bir makinede (örneğin önceki bölümdeki sunucuda) Kioku belirli saatlerde çalışıp sonucu sana gönderebilir:

| İş | Varsayılan saat | Ne yapar | Model çağrısı |
| --- | --- | --- | --- |
| `morning` | her gün 08:00 | Bugünün etkinlikleri ve vakti gelen yapılacaklar; inbox'ta bekleyenler ve eşitleme sorunları | Yok |
| `nightly` | her gün 03:00 | `kioku lint-check`; sorun bulursa Claude mekanik olanları düzeltir, kalanları bildirir | Yalnızca sorun varsa |
| `weekly` | pazar 20:00 | Haftanın değerlendirmesi (`kioku-review`): bitenler, takılanlar, kararlar, öğrenilenler, sıradakiler; `reviews/` altına kaydedilir | Haftada bir |

Gözetimsiz çalışmalar (`claude -p`) yalnızca okuyabilir, vault içindeki dosyaları düzenleyebilir, kioku CLI'ını ve `git add/commit` komutlarını çalıştırabilir; gerisi reddedilir, `Core.md`'ye de hiç dokunmazlar.

1. Ayarlar `~/.config/kioku/agents.conf` dosyasında (şifre içermez):
   ```ini
   notify=mail                # mail | command | stdout
   mail_to=sen@example.com    # varsayılan: sync.conf'taki user=
   smtp=smtp://127.0.0.1:25   # varsayılan: aynı makinedeki mail sunucusu
   lang=tr                    # en | tr: sabah özetinin dili
   # morning_at=*-*-* 08:00   nightly_at=…   weekly_at=Sun *-*-* 20:00   weekly=off
   ```
   Telegram, Discord ya da ntfy için `notify=command` ve `notify_command=/betik/yolu` yaz: betik konuyu `$KIOKU_SUBJECT` içinde, metni stdin'den alır.
2. Bir işi dene: `kioku agent morning --dry-run` göndermeden yazdırır; `kioku agent morning` gönderir.
3. `kioku agent setup` zamanlamayı kurar:
   - varsayılan olarak systemd kullanıcı zamanlayıcıları;
   - `agents.conf` içinde `scheduler=hermes` varsa bunun yerine [Hermes Agent](https://github.com/NousResearch/hermes-agent)'ın zamanlayıcısındaki işler (aşağıya bak); systemd zamanlayıcıları kaldırılır.

   systemd zamanlayıcıları için sunucuda lingering'i aç: `sudo loginctl enable-linger $USER`. `kioku agent status` sıradaki çalışmaları ve kaydı gösterir, `kioku agent remove` kaldırır.

### Hermes Agent ile

[Hermes Agent](https://github.com/NousResearch/hermes-agent) kuruluysa (`hermes` komutu varsa) kurulum Kioku'yu ona da bağlar:

- **Kurallar:** vault'taki `.hermes.md` dosyasına bir Kioku bölümü yazılır; Hermes vault'ta çalışırken bunu okur.
- **Skill'ler:** Kioku'nun skill'leri Hermes'in `skills.external_dirs` listesine eklenir.
- **Hazır skill'ler:** Hermes'in kendi `obsidian` ve `weekly-review-planning` skill'leri kapatılır, Kioku'nun işini başka kurallarla yapmasınlar diye.

`--no-hermes` bunu kapatır ve kurulanları kaldırır. `scheduler=hermes` ile `kioku agent setup`, vault'ta çalışan üç Hermes işi oluşturur:

| İş | Nasıl çalışır |
| --- | --- |
| `kioku-morning` | Betik işi; model çağrısı yok. |
| `kioku-nightly` | Önce bir lint kontrolü çalışır, ajan yalnızca sorun varsa uyanır ve `kioku-lint` ile düzeltir. |
| `kioku-weekly` | Hermes ajanı `kioku-review` ile çalışır. |

Sonuçlar `hermes_deliver=` hedefine (örneğin `discord`) gider; `notify=hermes` Kioku'nun kendi mesajlarını da oraya yollar. Hermes'in hangi modeli kullanacağı senin seçimin (`hermes model`); bu seçimin maliyeti ve kullanım koşulları da senin sorumluluğunda.

## Google Antigravity (isteğe bağlı)

Antigravity kuruluysa (`~/.gemini` varsa) kurulum Kioku'yu orada da kurar:

- **Kurallar:** `~/.gemini/GEMINI.md` içine `~/.claude/CLAUDE.md`'dekine benzer bir Kioku bölümü eklenir (dosyadaki kendi metnin korunur).
- **Skill'ler:** aynı skill'ler `~/.gemini/config/skills/` altına gelir; aynı cümleleri kullanabilir ya da `/kioku-journal` gibi çağırabilirsin.

- **Hook'lar:** `~/.gemini/config/hooks.json` içine `kioku-*` adlı üç girdi eklenir (oradaki diğer hook'lar korunur). Bunlar Claude Code'daki hook'ların aynısını `hooks/antigravity.sh` üzerinden çalıştırır; bu betik Antigravity'nin girdisini ve sohbet dökümünü çevirir:
  - `SessionStart` oturum bağlamını ekler;
  - `PreInvocation` autosync açıksa vault'u pull eder (en fazla 30 saniyede bir);
  - `Stop` Claude Code'daki gibi bir kez kayıt ister.
  - Yazma kontrolü ve sıkıştırma notu yalnızca Claude Code'da var: Antigravity'nin `PostToolUse` hook'u ajana bir şey iletemiyor, sıkıştırma hook'u da yok.

Hook'lar çalışmazsa kurallar ajana bağlamı `kioku context` ile kendisinin yüklemesini söyler.
Her hook'un son girdisi hata ayıklama için `~/.local/state/kioku/antigravity-*.json` dosyasında tutulur.

Kapatmak için `install.sh --no-antigravity` (Nix: `programs.kioku.antigravity = false;`); bu, kurulanları da kaldırır.

## Diller

Sistemin kendi metinleri İngilizce: şema, skill'ler, başlıklar, kayıt türleri, frontmatter.
Not içeriği kaynağına göre yazılır: bir makale kendi dilinde özetlenir, sohbet notları ise
konuştuğun dilde yazılır. Her sayfa dilini `lang:` alanında tutar. Arama Türkçe `İ/ı` ile
`I/i` harflerini aynı sayar.

## Ayarlar

- **Kişisel kurallar:** Claude'a söyle ("bundan sonra …"); onayınla vault'undaki `CLAUDE.md` dosyasının *Personal rules* bölümüne ekler.
- **Özel sayfalar:** `private: true` olan bir sayfa oturum başı bağlamına girmez.
- **Ortam değişkenleri**, `~/.claude/settings.json` içinde `env` altına yazılır:
  - `KIOKU_STOP_CHECK=0` Stop hook'unu ve sıkıştırma notunu kapatır.
  - `KIOKU_WRITE_CHECK=0` yazma kontrolünü kapatır.
  - `KIOKU_STOP_MIN_TOOLS=10` kaç araç çağrısının "gerçek iş" sayılacağını belirler.
  - `KIOKU_CONTEXT_MAX=12000` oturum başı bağlamın üst sınırını bayt olarak belirler.

## Güncelleme ve kaldırma

```bash
kioku update
```

Bu komut checkout'u pull eder, kurulum adımını kurarken kullandığın seçeneklerle yeniden çalıştırır ve nelerin değiştiğini listeler. `kioku update --check` yalnızca yeni commit var mı diye bakar; `kioku version` kurulu sürümü gösterir.

Güncellemeyi düz bir `git pull` ile yaparsan, kurulum adımı geride kaldığında (yeni skill'ler, ayarlar ya da taşıma adımları) bir sonraki oturum bunu fark eder ve Claude `kioku update` çalıştırmayı önerir. Nix modülünde `nix flake update kioku` çalıştırıp rebuild et; kurulum adımı her switch'te zaten çalışır.

```bash
~/Projects/kioku/install.sh --uninstall
```

Bu komut skill'leri, `kioku` komut bağlantısını, `~/.claude/CLAUDE.md` bölümünü ve ayar girdilerini kaldırır. Vault'un olduğu gibi kalır.

## Gizlilik

Her şey bilgisayarında ve kendi git deponda kalır. Şema sırların (şifre, anahtar, token,
kimlik ya da kart numarası) yazılmasını yasaklar. Vault'unu push edeceksen özel bir repo
kullan.

## Teşekkür

- Andrej Karpathy'nin [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)'si: raw / wiki / şema katmanları ve ingest, query, lint işlemleri.
- [avenox Beyin](https://github.com/avenoxai/avenoxbeyin): çekirdek bir profil ve günlük kayıtlarla Claude'un tuttuğu kişisel hafıza fikri.

## Lisans

[GPL-3.0](LICENSE)
