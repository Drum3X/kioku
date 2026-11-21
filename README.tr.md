# kioku

[English](README.md) | **Türkçe**

> **Beta.** Kioku beta aşamasında: sürümler ön sürüm (`v0.3.0-beta.N`) olarak yayınlanıyor ve aralarında hâlâ değişiklik olabilir.

AI ajanları için kişisel ikinci beyin: [Claude Code](https://claude.com/claude-code),
[Codex](https://developers.openai.com/codex), [Google Antigravity](https://antigravity.google),
[Hermes Agent](https://github.com/NousResearch/hermes-agent) ve dosya okuyup komut
çalıştırabilen her ajan. Sen konuşursun, notları ajan tutar.

- **Hafıza:** ajanın kim olduğunu, ne karar verdiğini ve neyle uğraştığını hatırlar; oturumlar ve projeler arasında.
- **Wiki:** verdiğin linkler ve PDF'ler, Andrej Karpathy'nin [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) desenine göre, büyümeye devam eden ve birbirine bağlı bir wikiye derlenir.
- **Journal:** kendi notların, yapılacakların ve fikirlerin; söylediğin anda sohbetten alınır.

Her şey sana ait bir klasörde düz Markdown. Klasörü [Obsidian](https://obsidian.md) vault'u
olarak açabilirsin ve git ile sürümlenir. Kendine ait bir sunucusu, veritabanı ya da API anahtarı
yok; CLI ve hook'lar küçük, derlenmiş tek bir program (`kioku`, Rust ile yazıldı). Aynı vault her ajanla çalışır: aracı değiştirsen ya da
birkaçını birlikte kullansan da tek bir hafızan olur.

## Nasıl çalışır

```
~/Documents/Kioku/            vault'un (özel, kendi git reponda)
  AGENTS.md                   her ajanı şemaya yönlendirir + kişisel kuralların
  CLAUDE.md                   Claude Code için iki satır: AGENTS.md'yi ve şemayı içe alır
  Core.md                     sen kimsin: "beni tanı" röportajıyla dolar
  daily/                      ajanın iş kaydı: kararlar, sonuçlar, öğrenimler, açık uçlar
  journal/                    zaman çizelgen: yaptıkların, yapılacaklar (tarihli), kaydettiklerin
  calendar/                   etkinlikler, her biri bir sayfa
  contacts/                   kişiler ve iletişim bilgileri
  projects/                   proje başına bir not: durum, sonraki adım, kararlar
  reviews/                    haftalık ya da aylık değerlendirmeler
  raw/                        işlenen kaynakların birebir kopyası (asla değiştirilmez)
  wiki/                       derlenen sayfalar: topics/ (kategori hub'ları) concepts/ entities/ ideas/ sources/ synthesis/
  index.md  log.md            beynin haritası (önce konular), yalnızca sona eklenen işlem kaydı
  Home.md                     Obsidian için başlangıç sayfası, her makinede üretilir (git'te değil)
  .kioku → kioku/             bu repoya bağlantı: şema, hook'lar, skill'ler, CLI, şablonlar
```

Üç parça her ajanla çalışır:

- **Şema ([SCHEMA.md](SCHEMA.md)):** ajanın uyduğu kurallar: ne nereye gider, diller, kaynak gösterme, asla yazılmayacaklar (sırlar), commit biçimi, güvenilmeyen içerik.
- **Skill'ler** (standart `SKILL.md` klasörleri):
  - `kioku` hafızayı kaydeder ve tanışma röportajını yapar;
  - `kioku-journal` zaman çizelgeni ve yapılacaklarını tutar;
  - `kioku-note` araçları, siteleri, fikirleri ve öğrendiklerini konu hub'larında kategorilenmiş, birbirine bağlı wiki sayfalarına çevirir;
  - `kioku-calendar` ve `kioku-contacts` etkinlikleri ve iletişim bilgilerini tutar;
  - `kioku-ingest` kaynak ekler;
  - `kioku-ask` kaynak göstererek cevap verir;
  - `kioku-lint` notların sağlık kontrolünü yapar;
  - `kioku-review` haftalık değerlendirme yazar ve planları sorgular.
- **CLI (`kioku`):** bağlam, arama, yapılacaklar, ajanda, yeni sayfa, kontroller, git eşitlemesi.

Bunların üstüne installer, makinede bulduğu her ajanı bağlar. Her biri şunları alır:

| | Claude Code | Codex | Antigravity | Hermes | Diğer ajanlar |
| --- | --- | --- | --- | --- | --- |
| Kurallar | `~/.claude/CLAUDE.md` | `~/.codex/AGENTS.md` | `~/.gemini/GEMINI.md` | vault'taki `.hermes.md` | vault'taki `AGENTS.md` |
| Skill'ler | `~/.claude/skills/` | `~/.agents/skills/` | `~/.gemini/config/skills/` | `skills.external_dirs` | `.kioku/skills/` klasörünü göster |
| Oturum başında bağlam | hook | hook | hook | `kioku context` | `kioku context` |
| Her mesajdan önce çekme (autosync) | hook | hook | hook | — | — |
| Yazma kontrolü | hook | hook | — | — | — |
| Sonda kayıt hatırlatması | hook | hook | hook | — | — |
| Sıkıştırmadan önce not | hook | — | — | — | — |

Hook'lar:

- **Oturum başı:** her oturum, hangi projede olursa olsun, yaklaşık 1k token'lık kısa bir bağlamla başlar: `Core.md`, bulunduğun klasörün notu, aktif projeler, yaklaşan yapılacaklar ve etkinlikler, iş kaydının son iki günü ve bir wiki özeti. Journal'ın, yaklaşan yapılacaklar dışında, bağlama girmez. Hook olmayan yerde kurallar ajana `kioku context`'i kendisinin çalıştırmasını söyler.
- **Her mesajdan önce:** autosync açıksa vault önce çekilir; başka makinede yazılan notlar görünür.
- **Yazma kontrolü:** ajan vault'a bir dosya yazar yazmaz frontmatter'ı, `Topics:` satırı, `[[bağlantılar]]` ve yapılacaklar/etkinlikler kontrol edilir; ajan hatayı aynı turda düzeltir.
- **Kayıt hatırlatması:** ajandan iki durumda, her biri için bir kez, kayıt istenir:
  - oturumda gerçek iş yapıldı ama hiçbir şey kaydedilmedi;
  - bir öğrenme iş kaydına yazıldı ama wikiye işlenmedi.

  Kısa soru-cevap oturumlarına karışılmaz.
- **Sıkıştırmadan önce:** uzun bir oturum sıkıştırılırken özet, henüz kaydedilmemiş olanları listeler; ajan hemen ardından onları kaydeder.

Şemanın ana fikri LLM Wiki'den geliyor: her soruda ham belgelerde arama yapmak (RAG) yerine
ajan bir kaynağı bir kez okur ve kavram ve varlık sayfalarına işler. Çelişkiler işaretlenir ve
her iddia kaynağına bağlı kalır.

## Gereksinimler

- Dosya okuyup kabuk komutu çalıştırabilen bir AI ajanı (yukarıdaki tabloya bak)
- `git`
- `kioku` ikilisi: installer onu Linux (x86_64, aarch64) ve macOS için indirir ya da `cargo` ile derler (Nix paketi de derler)
- Obsidian (isteğe bağlı): yalnızca gezinmek için

Linux'ta (NixOS ve Ubuntu) geliştiriliyor ve kullanılıyor. macOS ikilileri yayınlanıyor ama henüz denenmedi. Windows'ta WSL kullan.

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
    # claudeCode = false; codex = false; antigravity = false;   # bir ajana dokunmamak için
  };
}
```

Modül şöyle çalışır:
- **Komut:** `kioku`'yu ve `git`'i PATH'e koyar.
- **Bağlama:** her switch'te `kioku install` çalıştırır. Vault'taki `.kioku` bağlantısı Nix store'daki pakete gider, sürüm `flake.lock` dosyanda sabitlenir.
- **Güncelleme:** `nix flake update kioku`, ardından rebuild.

Ajanların dosyalarını (`~/.claude/settings.json`, `~/.codex/hooks.json`, `~/.codex/config.toml` …)
ajanların kendileri de düzenler. Modül bu dosyaları sahiplenmez; `install.sh` gibi kendi
girdilerini içlerine ekler.

**Kioku üzerinde çalışırken:** `programs.kioku.source = "/checkout/yolu";` ayarla. Vault o
checkout'a bağlanır, değişikliklerin rebuild gerekmeden etkili olur. Sabitlenmiş sürüme dönmek
için satırı sil.

**Kaldırma:** modülü kapatmak ajanlardaki bağlantıları kaldırmaz. Önce paketteki
`kioku-install --uninstall` komutunu çalıştır.

### Diğer sistemler

```bash
git clone https://github.com/Drum3X/kioku ~/Projects/kioku
```

```bash
~/Projects/kioku/install.sh
```

`install.sh` bu sürümün `kioku` ikilisini `bin/` içine indirir (GitHub sürümünden, sha256 doğrulanarak); platformun için sürüm yoksa `cargo` ile derler (`KIOKU_BUILD=1` her zaman derler), sonra `bin/kioku install` çalıştırır.

Checkout'u klonladığın yerde bırak: vault ona bağlanır. Varsayılan vault `~/Documents/Kioku`;
başka bir klasör ya da mevcut bir Obsidian vault'u için `--vault YOL` ver (notlarına
dokunulmaz). Installer:

1. vault klasörlerini ve başlangıç dosyalarını oluşturur. Var olan dosyaların üzerine asla yazmaz.
2. `vault/.kioku`'yu checkout'a bağlar; vault henüz bir repo değilse `git init` çalıştırır.
3. `kioku` komutunu `~/.local/bin/` içine bağlar (o klasör `PATH`'te değilse uyarır).
4. bulduğu her ajanı bağlar ([Ajanlar](#ajanlar) bölümüne bak); `--no-claude`, `--no-codex`, `--no-antigravity` ve `--no-hermes` bir ajana dokunmaz ve orada kurulanları kaldırır.

Tekrar çalıştırmak güvenli. Sonra ajanında yeni bir oturum aç ve **"beni tanı"** de.

### Windows

Kioku Windows 10 ve 11'de (x86_64) doğrudan çalışır. [Git for Windows](https://git-scm.com/download/win) gerekir; Claude Code ve Hermes'in de zaten ihtiyacı var, çünkü kabuk komutları ve vault'un git hook'u onun Git Bash'inde çalışır. PowerShell'de:

```powershell
git clone https://github.com/Drum3X/kioku $HOME\Projects\kioku
git clone <vault'unun reposu> $HOME\Documents\Kioku    # başka bir makinede tuttuğun vault
powershell -ExecutionPolicy Bypass -File $HOME\Projects\kioku\install.ps1 --vault $HOME\Documents\Kioku --host windows
```

- `install.ps1`, `install.sh`'nin yaptığını yapar: `bin\kioku.exe`'yi sürümden indirir (sha256 kontrolüyle) ya da cargo ile derler, sonra `kioku.exe install` çalıştırır. `kioku update` onu yeniden çalıştırır.
- **Bağlantılar:** Geliştirici Modu açıksa (Ayarlar › Sistem › Geliştiriciler için) Windows sembolik bağlantıları; değilse ek izin gerektirmeyen klasör bağlantıları (junction).
- **kioku komutu:** `%LOCALAPPDATA%\kioku\bin` içinde bir `kioku.cmd`; kurulum o klasörü PATH'e nasıl ekleyeceğini yazar.
- **Ayarlar ve durum:** `%APPDATA%\kioku` ve `%LOCALAPPDATA%\kioku` altında.
- **Hermes:** evi `%LOCALAPPDATA%\hermes` (ya da `HERMES_HOME`).
- **Dört ajan da** Linux'taki gibi kurulur: Claude Code, Codex, Antigravity ve Hermes.
- **Windows'ta henüz yok:** `kioku capture --gui` ve isteğe bağlı hibrit arama. Nix modülü yalnızca NixOS ve Home Manager için.

## Ajanlar

### Claude Code

`~/.claude` varsa ya da `claude` komutu bulunursa:

- **Kurallar:** `~/.claude/CLAUDE.md` içinde bir Kioku bölümü; vault'un kendi `CLAUDE.md`'si `AGENTS.md`'yi ve şemayı içe alır.
- **Skill'ler:** `~/.claude/skills/` içine bağlanır.
- **Hook'lar ve izinler:** beş hook'un hepsi, vault için okuma/düzenleme izinleri ve bir sandbox yazma kuralı `~/.claude/settings.json` dosyasına eklenir. Yedeği `settings.json.kioku.bak` olarak saklanır.

### Codex

`~/.codex` (ya da `$CODEX_HOME`) varsa veya `codex` komutu bulunursa:

- **Kurallar:** `~/.codex/AGENTS.md` içinde bir Kioku bölümü. Codex vault'un içindeyken vault'un `AGENTS.md`'sini de kendisi okur.
- **Skill'ler:** Codex'in ve başka ajanların okuduğu ortak klasöre, `~/.agents/skills/` içine bağlanır.
- **Hook'lar:** `~/.codex/hooks.json` dosyasına dört girdi (oradaki diğer hook'lar korunur), `kioku hook codex …` olarak çalışır:
  - `SessionStart` oturum bağlamını ekler;
  - `UserPromptSubmit` autosync ile vault'u çeker;
  - `PostToolUse` oturumun araç çağrılarını sayar ve `apply_patch` ile değişen dosyalarda yazma kontrolünü yapar;
  - `Stop` bir kez kayıt ister, ardından vault'u push eder.

  Codex hook'ları ancak sen onlara güvendikten sonra çalıştırır: yeni bir oturumda bir kez `/hooks` çalıştırıp Kioku'nunkileri onayla.
- **Sandbox:** vault, `~/.codex/config.toml` içindeki `writable_roots`'a eklenir; böylece Codex her projeden vault'a yazabilir. Dosyada zaten bir `[sandbox_workspace_write]` tablosu varsa installer ona dokunmaz ve vault'u oraya senin eklemeni ister. Codex'in sandbox'ındaki komutların ağ erişimi yoktur; orada yapılan commit'i turun sonunda `Stop` hook'u push eder.
- **Yok:** sıkıştırmadan önceki not (Codex'in `PreCompact` hook'u özete talimat ekleyemiyor).

Codex'in konuşma kaydı biçimi kararlı bir arayüz değil; bu yüzden kayıt hatırlatması onu okumaz:
araç hook'u ihtiyaç duyduğunu `~/.local/state/kioku/codex/` altında tutar.

### Google Antigravity

`~/.gemini` varsa ya da `antigravity` komutu bulunursa:

- **Kurallar:** `~/.gemini/GEMINI.md` içinde bir Kioku bölümü (o dosyadaki kendi metnin korunur).
- **Skill'ler:** `~/.gemini/config/skills/` içinde; aynı şeyleri söyle ya da `/kioku-journal` gibi çağır.
- **Hook'lar:** `~/.gemini/config/hooks.json` dosyasında `kioku-*` adlı üç girdi (oradaki diğer hook'lar korunur); Antigravity'nin girdisini ve konuşma kaydını çeviren `kioku hook antigravity …` olarak çalışır:
  - `SessionStart` oturum bağlamını ekler;
  - `PreInvocation` autosync ile vault'u çeker (en fazla 30 saniyede bir);
  - `Stop` bir kez kayıt ister.
- **Yok:** yazma kontrolü ve sıkıştırmadan önceki not: Antigravity'nin `PostToolUse` hook'u ajana bir şey geri gönderemiyor, sıkıştırma hook'u da yok.

Hook'lar çalışmazsa kurallar ajana bağlamı `kioku context` ile kendisinin yüklemesini söyler.
Her hook'un son girdisi hata ayıklamak için `~/.local/state/kioku/antigravity-*.json` dosyasında tutulur.

### Hermes Agent

`hermes` komutu bulunursa:

- **Kurallar:** vault'taki `.hermes.md` dosyasında bir Kioku bloğu; Hermes vault'ta çalışırken bunu okur.
- **Skill'ler:** Kioku'nun skill'leri Hermes'in `skills.external_dirs` listesine eklenir.
- **Hazır skill'ler:** Hermes'in kendi `obsidian` ve `weekly-review-planning` skill'leri kapatılır; Kioku'nun işini başka kurallarla yapmasınlar diye.
- **Eklenti:** `~/.hermes/plugins/kioku` (checkout'taki `hermes-plugin/kioku` klasörüne bir bağlantı) `plugins.enabled` listesine eklenir. Diğer ajanlarda hook'ların yaptığını Telegram ve Discord dahil her Hermes oturumunda yapar: ilk turda ve sıkıştırmadan sonra oturum bağlamı, Hermes'in vault'a yazdığı her dosyanın kontrolü (sorunlar aracın sonucuna eklenir, Hermes aynı turda düzeltir) ve biten işi kaydetme hatırlatması. Kurulumdan sonra gateway'i yeniden başlat (`systemctl --user restart hermes-gateway`).

Kioku'nun kendi zamanlayıcısı yok. Hermes ile vault'ta çalışan kendi işlerini ekleyip sonucu
Telegram'a, Discord'a vb. gönderebilirsin. Birkaç örnek:

```bash
# Model çağırmayan sabah özeti: çıktısını Hermes olduğu gibi gönderen bir betik
cat > ~/.hermes/scripts/morning.sh <<'EOF'
#!/usr/bin/env bash
cd ~/Documents/Kioku && .kioku/bin/kioku remote pull
echo "Bugün:"; .kioku/bin/kioku agenda --days 1
echo; echo "Yaklaşan yapılacaklar:"; .kioku/bin/kioku todo --soon
EOF
chmod +x ~/.hermes/scripts/morning.sh
hermes cron create "0 8 * * *" --name morning --no-agent --script morning.sh --deliver telegram

# kioku-review skill'iyle haftalık değerlendirme
hermes cron create "0 20 * * 0" "Bu vault'un haftalık değerlendirmesini kioku-review skill'iyle yap: sayfayı yaz, commit et, sonra .kioku/bin/kioku remote now çalıştır. Değerlendirmeyi cevap olarak yaz." \
  --name weekly --skill kioku-review --workdir ~/Documents/Kioku --deliver telegram
```

### Diğer ajanlar

`AGENTS.md` okuyan her ajan (OpenCode, Cursor, Aider, Gemini CLI …) vault'ta açıldığında
kuralları bulur. Başka projelerden kullanmak için ajanın genel talimat dosyasına bir satır ekle,
örneğin: *"İkinci beynim ~/Documents/Kioku. Her konuşmanın başında
`~/Documents/Kioku/.kioku/bin/kioku context` çalıştır; oraya yazmadan önce AGENTS.md ve
.kioku/SCHEMA.md dosyalarını oku."* Ajan `SKILL.md` skill'lerini destekliyorsa skill klasörünü
`~/Documents/Kioku/.kioku/skills/` olarak göster; desteklemiyorsa onları dosya olarak okuyabilir.
Geri kalan her şey aynı çalışır; yalnızca hook'lar eksik kalır.

Bir ajanın hangi modeli kullanacağı senin seçimin; bu seçimin maliyeti ve kullanım koşulları
senin sorumluluğunda.

## Kullanım

Normal konuş, hangi dilde istersen. Birkaç örnek:

| Sen dersin | Ne olur |
| --- | --- |
| "beni tanı" | Sen dur diyene kadar, her seferinde bir soru soran açık uçlu bir röportaj; sonra onayına sunulan bir `Core.md` taslağı |
| *(ajanınla bir işi bitirirsin)* | `daily/` ve proje notuna bir satır, sonra `Kioku'ya kaydedildi: …` |
| "bunu beyne at: https://…" | `raw/` altına kaydedilir, ana noktalar konuşulur, özet + kavram/varlık sayfaları, index, log |
| "inbox'ı işle" | `raw/inbox/`'ta bekleyen her kaynak, tek tek ve her biri ayrı commit'le; durup yeniden istersen kaldığı yerden devam eder |
| "X hakkında ne biliyoruz?" | Wikiden `[[kaynaklarla]]` cevap; beyinde bir şey yoksa bunu söyler |
| "Config'le devam edelim" / "sunucuda nerede kalmıştık?" | `kioku context --deep`: önce eski kararlar, açık işler ve son yapılanlar, sonra iş |
| "not al: bugün sınavı bitirdim" / "cuma hatırlat: ödevi teslim et" | Bugünün `journal/` dosyasına bir satır (göreli tarihler gerçek tarihe çevrilir) |
| "yapılacaklarım ne?" / "ödevi yaptım" | Gruplanmış liste / yapılacak yerinde işaretlenir |
| "bunu kaydet: Ghostty, denemek istediğim GPU terminali" / "fikir: küçük bir zaman döngüsü oyunu" / "şunu öğrendim: `nix why-depends` bağımlılık zincirini gösteriyor" | Her biri için bir wiki sayfası (`entities/`, `ideas/`, `concepts/`), *Linux › Araçlar* gibi bir konu hub'ına eklenir; uygun hub yoksa yenisi açılır ve söylenir |
| "cuma 14:00 diş randevusu" / "her pazartesi 10:00 ders" / "yarınki toplantıyı iptal et" | `calendar/` altında bir sayfa, ajandada görünür |
| "Ahmet'i rehbere ekle, numarası …" | `contacts/` altında bir sayfa |
| "bu hafta nasıl geçti?" / "bu planı sorgula" | `reviews/` altında bir değerlendirme / plan geçmiş kararlara göre sorgulanır |
| "beyni kontrol et" | Önce lint raporu, düzeltmeler yalnızca onayından sonra; birkaç eski bilginin hâlâ doğru olup olmadığını sorar |
| *(terminalde ya da bir kısayolla)* `kioku capture "yarın bankayı ara"` | Oturum açmadan `raw/inbox/` içine düşer; sonraki oturum işlemeyi önerir |

`Core.md` değişiklikleri her zaman onayını ister. Geri kalan her şey sormadan kaydedilir ve her
yazma vault'unda bir git commit'idir. Ajan push etmez; birden fazla makine için `kioku remote`'u
açarsan hariç.

CLI'ı elle de `kioku` komutuyla kullanabilirsin (`vault/.kioku/bin/kioku` dosyasına bir bağlantı):

```
kioku health | capture "<metin>" | context [--agent AD | --deep <proje|konu>] | whoami | today | journal | todo [--soon] | agenda [--days N] [--past] | new <tür> "<Başlık>" | log <işlem> "<Başlık>"
kioku search [--raw] [-n N] <terim...> | eval [-v] [DOSYA.tsv ...] | recap [--days N] | stale [--all] | home [--print] | remote on|off|status|now|setup | update [--check|--to vX|--rollback] | version | lint-check
```

## Hızlı yakalama ve sorun giderme

- **`kioku capture "metin ya da link"`** bir düşünceyi, yapılacağı ya da linki oturum açmadan `raw/inbox/` klasörüne atar (dosya için `kioku capture --file F`, stdin için `kioku capture -`). Commit edildiği için autosync açıksa diğer makinelere de gider. Sonraki oturum kaç tane beklediğini söyler ve işlemeyi önerir: notlar sanki sohbette söylenmiş gibi journal'a, wikiye ya da takvime gider; linkler ve dosyalar ingest edilir. Klavye kısayolu için `kioku capture --gui` küçük bir pencerede sorar (kdialog ya da zenity); Nix modülünde `programs.kioku.capture.gui = true;` bu araçları ve kısayol atanabilecek bir "Kioku capture" masaüstü girdisini getirir.
- **`Home.md`** vault'u Obsidian'da gezmek için bir başlangıç sayfası: önümüzdeki haftanın etkinlikleri, yaklaşan yapılacaklar, sıradaki adımlarıyla aktif projeler, açık işler, bu hafta değişen sayfalar, gözden geçirilecek sayfalar ve inbox; her biri kendi sayfasına bağlı. Üretilir (oturum başında, her commit'ten ve pull'dan sonra ya da `kioku home` ile) ve git dışında tutulur, yani her makinenin kendi kopyası olur; elle düzenleme.
- **Kim yazdı:** tek vault'a birden fazla ajan ve makine yazdığı için her iş kaydı satırı projesinin yanında `ajan@makine` taşır (`- 21:55 [brain · claude@nitro] done: …`) ve her commit `Kioku-Agent: claude@nitro` satırıyla biter. Oturum bağlamı her ajana adını söyler; `kioku whoami` onu yazar. Makine adı kurulumda verilen addır (`kioku install --host laptop`, sonraki kurulumlarda korunur; Nix modülünde `host` seçeneği), verilmediyse bilgisayarın adı; `KIOKU_HOST` ikisinin de önüne geçer. Proje notları yaşadıkları makineleri `hosts:` alanında tutar (`hosts: [laptop, server]`); oturum bağlamı bunu her aktif projenin yanında gösterir ve eksik bir makineyi eklemeyi söyler.
- **`kioku context --deep <proje ya da konu>`** bir projeye ya da konuya geri dönmek için onunla ilgili her şeyi toplar: notun kendisi, iş kaydındaki kararlar, açık işler, öğrenmeler ve son yapılanlar (yeniden eskiye), bağlantılı ve bağlanan sayfalar, ilgili yapılacaklar ve etkinlikler. Ajan bunu bir projeye döndüğünde çalıştırır ("Config'le devam edelim"); oturum bir projenin klasöründe başlarsa bağlam bu komutu hatırlatır.
- **`kioku eval`** aramanın doğru sayfayı ne kadar iyi bulduğunu ölçer. Vault'taki `eval/*.tsv` (kendi soruların, özel vault'unda kalır) ve checkout'taki `tests/queries/*.tsv` dosyalarının her satırında bir soru, ajanın arayacağı kelimeler ve doğru sayfa(lar) var. Her soru hem bu kelimelerle hem sorunun tamamıyla aranır; doğru sayfanın kaç soruda ilk sırada, ilk 3'te ve ilk 10'da geldiğini, ortalama sırayı (MRR) ve kaçırılan soruları yazar. Doğru sayfası boş bırakılan satırlar vault'ta karşılığı olmayan sorulardır: arama boş dönmeli. Aramayı değiştirmeden önce ve sonra kendi sorularınla çalıştır (`kioku eval sorularim.tsv`).
- **Aramada zaman:** "geçen hafta", "dün", "2 hafta sonra", "last week" gibi ifadeler tarihe çevrilir. O döneme ait sayfalar (günlükler, journal günleri, haftalık ve aylık değerlendirmeler, takvim etkinlikleri) önce gelir, değerlendirmeler en önde.
- **Hibrit arama (isteğe bağlı, varsayılan olarak kapalı):** `kioku search` vault'un bölümlerini multilingual-e5-small modeliyle anlamına göre de sıralayıp kelime sıralamasıyla birleştirebilir. Başka kelimelerle ya da öbür dilde sorulan sayfaları bulur: yazarın kendi sorularında doğru sayfanın ortalama sıra skoru (MRR) 0,55'ten 0,67'ye çıktı. Sürüm ikilileri bu özellik olmadan derlenir. Kullanmak için:
  ```sh
  cargo build --release --features semantic   # çalışırken ONNX Runtime kütüphanesi gerekir
  export KIOKU_ORT_LIB=/yol/libonnxruntime.so
  kioku semantic pull                          # model, ~135 MB, sağlama toplamı kontrol edilir
  kioku semantic on                            # kioku semantic off yeniden kapatır
  ```
  Bir arama o zaman yaklaşık bir saniye sürer (model her seferinde yüklenir); bölüm vektörleri `~/.cache/kioku/` altında tutulur, yalnızca değişen bölümler yeniden hesaplanır. `kioku semantic status` neyin eksik olduğunu söyler; eksik bir şey varsa arama eskisi gibi çalışır.
- **Yerel hafızalar:** her ajanın diğerlerinin göremediği küçük bir hafızası da var (Claude Code'un otomatik hafızası, Hermes'in MEMORY.md ve USER.md dosyaları, Codex'in hafızası). `kioku memories` orada olup vault'ta olmayanları listeler; ajan her girdinin nereye ait olduğunu önerir (Core.md ve AGENTS.md yalnızca onayınla) ve işlendi olarak işaretler. Yerel dosyalar yalnızca okunur.
- **Düzeltmeler kurala dönüşür:** bir ajanı düzelttiğinde ajan bir `correction:` satırı yazar. Aynı düzeltme hangi ajanda olursa olsun ikinci kez geldiğinde yazma kontrolü bunu söyler ve ajan `AGENTS.md`'ye (ya da `Core.md`'ye) kalıcı bir kural önerir; kural yalnızca onayınla yazılır, satırlar `ruled:` ya da `declined:` olarak işaretlenir. `kioku corrections` ve `kioku lint-check` henüz kurala dönüşmemiş tekrarları listeler.
- **Gizli bilgi kontrolü:** yazma kontrolü, `kioku lint-check` ve vault'un pre-commit hook'u API anahtarlarını ve token'ları (Anthropic, OpenAI, GitHub, Google, AWS, Slack, Telegram), özel anahtar bloklarını, JWT'leri, `password = …` biçimindeki değerleri ve kart numaralarını arar. Bunlardan birini ekleyecek bir commit reddedilir (`kioku check-secrets --staged` nerede olduğunu söyler). Örnek bir biçimi anlatan satıra `kioku:allow-secret` eklenebilir.
- **`kioku stale`** bilgileri bir süredir kontrol edilmemiş wiki ve proje sayfalarını listeler: varsayılan olarak aktif projeler 60, araçlar, siteler ve servisler 90, kişiler ve kavramlar 180 gün sonra (anime, oyun gibi şeyler hiç) (`review_every:` bunu sayfa başına değiştirir, `reviewed:` son kontrolü tutar). Oturum bağlamı bunları sayar, `kioku lint-check` sorun saymadan listeler; ajan her seferinde birkaçını sana sorar ("Bu hâlâ doğru mu?"), sonra `reviewed:` alanını günceller ya da sayfayı düzeltir.
- **`kioku health`** Kioku'nun dayandığı her şeyi kontrol eder (araçlar, vault bağlantısı ve kuralları, her ajanın kuralları, skill'leri ve hook'ları, git ve autosync, vault sağlığı) ve her sorun için ne yapılacağını söyler. `--offline` ağ kontrolünü atlar.

## Takvim, rehber ve yapılacaklar

Etkinlikler (`calendar/`), kişiler (`contacts/`) ve journal'daki yapılacaklar vault'ta not
olarak durur. Oturum bağlamı ve `kioku agenda` / `kioku todo --soon` yaklaşanları gösterir.
Kioku başka takvimlere bir şey göndermez ve kendisi hatırlatma çalmaz; istersen bunu üstüne
kurabilirsin, örneğin bir Hermes cron işiyle ([Hermes Agent](#hermes-agent) bölümüne bak).

## Birden fazla makine ve telefon (isteğe bağlı)

Vault'un bir kopyasını sürekli açık bir sunucu gibi ikinci bir makinede tutup telefonundan
kullanabilirsin; örneğin Claude Code'un [Remote Control](https://code.claude.com/docs/en/remote-control)
özelliğiyle ya da Telegram veya Discord'daki Hermes ile.

1. Vault'u bir kez özel bir git remote'a push et: `git -C ~/Documents/Kioku push -u origin HEAD`.
2. Her makinede `kioku remote on` çalıştır. Bundan sonra:
   - her oturum bir `git pull` ile başlar ve her mesajda yeniden çeker (en fazla 30 saniyede bir); böylece zaten açık olan bir oturum başka yerde yazılan notları görür;
   - her commit arka planda çekilir, rebase edilir ve push edilir.
3. Sunucuda: bu repoyu ve vault'u klonla, `install.sh` ve ardından `kioku remote on` çalıştır; sonra ajanını orada başlat (örneğin vault içinde, tmux'ta ya da bir servis olarak `claude remote-control`).

İki makine aynı daily ya da journal dosyasına satır eklerse git iki tarafı da korur
(`.gitattributes` içinde `merge=union`). Gerçek bir çakışma asla zorlanmaz: yerel commit'ler
kalır, hiçbir şey push edilmez ve sonraki oturum sana bunu söyler. Sunucu UTC'de çalışıyorsa
`TZ` ayarla; böylece "bugün" ve etkinlik saatleri seninkiyle aynı olur. Durumu `kioku remote status`
gösterir, kayıtlar `~/.local/state/kioku/git-sync.log` dosyasına gider.

## Diller

Sistem metni İngilizce: şema, skill'ler, başlıklar, kayıt türleri, frontmatter. Not içeriği
kaynağını izler: bir makale kendi dilinde özetlenir, sohbet notları konuştuğun dilde yazılır.
Her sayfa dilini `lang:` alanında tutar. Arama sonuçları sıralar (başlık, alt başlıklar, nadir
kelimeler ve wiki sayfaları daha çok sayılır), Türkçe `İ/ı`, `I/i`, `ç/c`, `ş/s` gibi harfleri
aynı sayar ve ekleri tolere eder: "açılışta", "açılış"ı bulur.

## Ayarlar

- **Kişisel kurallar:** ajanına söyle ("bundan sonra …"); onayınla vault'undaki `AGENTS.md` dosyasında *Personal rules* altına ekler.
- **Özel sayfalar:** `private: true` bir sayfayı oturum başı bağlamının dışında tutar.
- **Ortam değişkenleri** (hook'lar için; Claude Code'da `~/.claude/settings.json` içinde `env` altına, diğer ajanlarda başladıkları ortama):
  - `KIOKU_STOP_CHECK=0` kayıt hatırlatmasını ve sıkıştırma notunu kapatır.
  - `KIOKU_WRITE_CHECK=0` yazma kontrolünü kapatır.
  - `KIOKU_STOP_MIN_TOOLS=10` kaç araç çağrısının "gerçek iş" sayılacağını belirler.
  - `KIOKU_CONTEXT_MAX=9500` oturum başı bağlamını karakter olarak sınırlar (Claude Code bir hook'un bağlamını yalnızca 10.000 karaktere kadar gösterir). Her bölümün kendi bütçesi var: Core 3.500, klasörün notu 2.000, devirler 1.200, aktif projeler 1.000, yapılacaklar 900, etkinlikler 700; son iki günün iş kaydı kalan yeri alır (en fazla 4.000). Bütçesini aşan bölüm kısaltılır, hiçbir zaman satır ortasından kesilmez: kararlar, açık işler, devirler ve tercihler tam kalır, uzun satırlar bağlantıları korunarak kısaltılır, en eski sıradan satırlar ise nereden okunacağını söyleyen bir notla çıkarılır. `kioku context --budget` her bölümün boyutunu gösterir; `kioku health` Core.md bütçesine yaklaşınca uyarır.

## Geliştirme

Kaynak `src/` altında (tek bir Rust crate'i; ağ ya da async bağımlılığı yok; isteğe bağlı `semantic` özelliği ONNX Runtime ve bir tokenizer ekler). Nix ile `nix develop`
cargo, clippy ve rustfmt verir. `cargo build --release` `target/release/kioku`'yu derler;
`install.sh` kaynaktan derlediğinde `bin/kioku`'yu ona bağlar, böylece vault'un bağlı olduğu
checkout senin derlemeni çalıştırır. `cargo test` birim testlerini çalıştırır;
`tests/make-fixture.sh KLASÖR` komutları denemek için zor durumlar içeren bir test vault'u kurar
(tekrarlayan ve bozuk etkinlikler, geçersiz tarihler, kırık bağlantılar, özel sayfalar).
`tests/e2e.sh DOSYA.tsv …` uçtan uca testtir: her eval sorusu yeni ve salt okunur bir Claude Code
oturumuna (`claude -p`) gider; oturum doğru sayfalardan birine dayanarak cevap vermeli, doğru sayfa
yoksa beyinde olmadığını söylemeli. Aboneliğinin token'larını harcar, o yüzden sürümden önce çalıştır. Bir `v*`
etiketi sürüm ikililerini derleyip yayınlar (`.github/workflows/release.yml`).

## Güncelleme ve kaldırma

```bash
kioku update
```

Bu komut checkout'u çeker, kurulum adımını kurduğun seçeneklerle yeniden çalıştırır ve neyin
değiştiğini listeler. `kioku update --check` yalnızca yeni commit var mı diye bakar,
`kioku version` kurulu sürümü yazar.

Bir güncelleme bir şeyi bozarsa `kioku update --rollback` güncellemeden önceki sürüme,
`kioku update --to v0.2.1` istediğin bir sürüme döner. O sürümün ikilisi indirilir ve kurulum
adımı çalışır; hook'lar ve kurallar da ona uyar. Checkout orada sabit kalır (`kioku health`
hatırlatır), ta ki düz bir `kioku update` seni yine en son sürüme taşıyana kadar. Notlarına
dokunulmaz. Nix modülünde bunun yerine flake girdisini sabitle:
`nix flake lock --override-input kioku github:Drum3X/kioku/v0.2.1`, sonra rebuild.

Bunun yerine düz `git pull` ile güncellersen, sonraki oturum kurulum adımının eskidiğini
(yeni skill'ler, ayarlar ya da taşımalar) fark eder ve ajan `kioku update` çalıştırmayı önerir.
Nix modülünde `nix flake update kioku` çalıştırıp rebuild yap; kurulum adımı her switch'te
çalışır.

```bash
~/Projects/kioku/install.sh --uninstall
```

Bu komut `kioku` komut bağlantısını ve her ajandaki skill'leri, kural bölümlerini ve hook
girdilerini kaldırır. Vault'un yerinde kalır.

## Gizlilik

Notların kendi makinende ve kendi git reponda kalır; ajanın onlardan okuduğu kısım, konuşmanın
geri kalanı gibi, kullandığın ajanın model sağlayıcısına gider. Şema sırların (şifreler,
anahtarlar, token'lar, kimlik ya da kart numaraları) yazılmasını yasaklar. Vault'unu push
ediyorsan özel bir repo kullan.

## Teşekkür

- Andrej Karpathy'nin [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)'si: raw / wiki / şema katmanları ve ingest, query ve lint işlemleri.
- [avenox Beyin](https://github.com/avenoxai/avenoxbeyin): çekirdek profil ve günlük kayıtlarla, ajanın tuttuğu kişisel hafıza fikri.

## Lisans

[GPL-3.0](LICENSE)
