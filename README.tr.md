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
  calendar/                   etkinlikler, her biri bir sayfa
  contacts/                   kişiler ve iletişim bilgileri
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
| "cuma 14:00 diş randevusu" / "her pazartesi 10:00 ders" / "yarınki toplantıyı iptal et" | `calendar/` altında bir sayfa, ajandada görünür |
| "Ahmet'i rehbere ekle, numarası …" | `contacts/` altında bir sayfa |
| "bu hafta ne yaptım?" | `kioku recap` özeti |
| "beyni kontrol et" | Önce bir lint raporu, düzeltmeler ancak senin onayınla |
| *(terminalde ya da kısayolla)* `kioku capture "yarın bankayı ara"` | Oturum açmadan `raw/inbox/` klasörüne düşer; sonraki oturum işlemeyi önerir |

`Core.md` değişiklikleri her zaman senin onayını ister. Geri kalan her şey sormadan
kaydedilir ve her yazım vault'unda bir git commit'i olur. Claude, birden fazla makine için `kioku remote`'u açmadıysan asla push yapmaz; o sana kalmış.

CLI'ı elle de `kioku` komutuyla kullanabilirsin (`vault/.kioku/bin/kioku` dosyasına bir bağlantı):

```
kioku health | capture "<metin>" | context | today | journal | todo [--soon] | agenda [--days N] [--past] | new <tür> "<Başlık>" | log <işlem> "<Başlık>"
kioku search [--raw] [-n N] <terim...> | recap [--days N] | remote on|off|status|now|setup | update [--check] | version | lint-check
```

## Hızlı yakalama ve sorun giderme

- **`kioku capture "metin ya da link"`** bir düşünceyi, yapılacağı ya da linki oturum açmadan `raw/inbox/` klasörüne atar (dosya için `kioku capture --file F`, stdin için `kioku capture -`). Commit edildiği için autosync açıksa diğer makinelere de gider. Sonraki oturum kaç tane beklediğini söyler ve işlemeyi önerir: notlar sanki sohbette söylenmiş gibi journal'a, wikiye ya da takvime gider; linkler ve dosyalar ingest edilir. Klavye kısayolu için `kioku capture --gui` küçük bir pencerede sorar (kdialog ya da zenity); Nix modülünde `programs.kioku.capture.gui = true;` bu araçları ve kısayol atanabilecek bir "Kioku capture" masaüstü girdisini getirir.
- **`kioku health`** Kioku'nun dayandığı her şeyi kontrol eder (araçlar, vault bağlantısı, hook'lar, skill'ler, Antigravity, git ve autosync, vault sağlığı) ve her sorun için ne yapılacağını söyler. `--offline` ağ kontrolünü atlar.

## Takvim, rehber ve yapılacaklar

Etkinlikler (`calendar/`), kişiler (`contacts/`) ve journal'daki yapılacaklar vault'ta not
olarak durur. Oturum bağlamı ve `kioku agenda` / `kioku todo --soon` yaklaşanları gösterir.
Kioku başka takvimlere bir şey göndermez ve kendisi hatırlatma çalmaz; istersen bunu üstüne
kurabilirsin, örneğin bir Hermes cron işiyle (aşağıya bak).
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

## Hermes Agent (isteğe bağlı)

[Hermes Agent](https://github.com/NousResearch/hermes-agent) kuruluysa (`hermes` komutu varsa) installer Kioku'yu ona da bağlar:

- **Kurallar:** vault'taki `.hermes.md` dosyasında bir Kioku bloğu; Hermes vault'ta çalışırken bunu okur.
- **Skill'ler:** Kioku'nun skill'leri Hermes'in `skills.external_dirs` listesine eklenir.
- **Hazır skill'ler:** Hermes'in kendi `obsidian` ve `weekly-review-planning` skill'leri kapatılır; Kioku'nun işini başka kurallarla yapmasınlar diye.

`--no-hermes` bunu kapatır ve kurulanları kaldırır.

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

Hermes'in hangi modeli kullanacağı senin seçimin (`hermes model`); bu seçimin maliyeti ve kullanım koşulları senin sorumluluğunda.
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
konuştuğun dilde yazılır. Her sayfa dilini `lang:` alanında tutar. Arama sonuçları
sıralar (başlık, alt başlıklar, nadir kelimeler ve wiki sayfaları daha çok sayılır), Türkçe
`İ/ı`, `I/i`, `ç/c`, `ş/s` gibi harfleri aynı sayar ve ekleri tolere eder: "açılışta",
"açılış"ı bulur.

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
