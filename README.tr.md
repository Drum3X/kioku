# second-brain

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
~/Documents/Brain/            vault'un (özel, kendi git deposun)
  CLAUDE.md                   şemayı içe alır + kişisel kuralların
  Core.md                     sen kimsin: "beni tanı" röportajıyla dolar
  daily/                      Claude'un iş kaydı: kararlar, sonuçlar, öğrenimler, açık uçlar
  journal/                    zaman çizelgen: yaptıkların, yapılacaklar (tarihli), kaydettiklerin
  projects/                   proje başına bir not: durum, sonraki adım, kararlar
  raw/                        işlenen kaynakların birebir kopyası (asla değiştirilmez)
  wiki/                       derlenen sayfalar: topics/ (kategori hub'ları) concepts/ entities/ ideas/ sources/ synthesis/
  index.md  log.md            beynin haritası (önce konular), yalnızca sona eklenen işlem kaydı
  .brain → second-brain/      bu repoya bağlantı: şema, hook'lar, skill'ler, CLI, şablonlar
```

Claude Code'a dört noktadan bağlanır:

- **SessionStart hook'u:** hangi projede olursa olsun her oturum yaklaşık 1k token'lık kısa bir bağlamla başlar: `Core.md`, o klasörün notu, aktif projeler, vakti gelen yapılacaklar, iş kaydının son iki günü ve bir wiki özeti. Günlüğün bu bağlama girmez; tek istisna vakti gelen yapılacaklardır.
- **Stop hook'u:** bir oturumda gerçek bir iş yapılmış ama hiçbir şey kaydedilmemişse, Claude'dan bir kez önemli olanı kaydetmesi istenir. Kısa soru-cevap oturumlarına karışmaz.
- **Skill'ler:**
  - `brain` hafızayı kaydeder ve tanışma röportajını yürütür;
  - `brain-journal` zaman çizelgeni ve yapılacaklarını tutar;
  - `brain-note` araçları, siteleri, fikirleri ve öğrendiklerini konu hub'larında kategorize edilmiş, birbirine bağlı wiki sayfalarına dönüştürür;
  - `brain-ingest` kaynak ekler;
  - `brain-ask` kaynak göstererek cevap verir;
  - `brain-lint` sağlık kontrolü yapar.
- **Şema ([SCHEMA.md](SCHEMA.md)):** Claude'un uyduğu kurallar: neyin nereye yazılacağı, diller, kaynak gösterme, asla yazılmayacaklar (sırlar), commit biçimi.

Şemanın ana fikri LLM Wiki'den geliyor: her soruda ham belgeleri yeniden aramak (RAG)
yerine, Claude bir kaynağı bir kez okur ve kavram ve varlık sayfalarına işler. Çelişkiler
işaretlenir ve her iddia kaynağına bağlı kalır.

## Gereksinimler

- Claude Code
- bash 4+, `jq`, `ripgrep`, `git`
- `node` (isteğe bağlı): Latin dışı başlıklardan dosya adı üretmek için (`машинное-обучение.md`, `機械学習.md`)
- Obsidian (isteğe bağlı): sadece gezinmek için

Linux'ta (NixOS) geliştirildi ve kullanılıyor. macOS'ta daha yeni bir bash ile çalışması beklenir (`brew install bash jq ripgrep`). Windows'ta WSL kullan. macOS ve Windows henüz test edilmedi.

## Kurulum

```bash
git clone https://github.com/Drum3X/second-brain ~/Projects/second-brain
```

```bash
~/Projects/second-brain/install.sh
```

Klonladığın klasörü yerinde bırak, çünkü vault ona bağlanıyor. Varsayılan vault
`~/Documents/Brain`. Başka bir klasör ya da mevcut bir Obsidian vault'u için `--vault YOL`
kullan; mevcut notlarına dokunulmaz. Kurulum betiği şunları yapar:

1. vault klasörlerini ve başlangıç dosyalarını oluşturur. Var olan dosyaları asla ezmez.
2. `vault/.brain`'i bu klasöre bağlar; vault henüz bir git deposu değilse `git init` çalıştırır.
3. skill'leri `~/.claude/skills/` altına bağlar.
4. `~/.claude/CLAUDE.md` dosyasına kısa bir Brain bölümü ekler.
5. `~/.claude/settings.json` dosyasına hook'ları, vault için okuma/düzenleme izinlerini ve bir sandbox yazma kuralını ekler. Yedeği `settings.json.brain.bak` olarak saklanır.

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
| "bu hafta ne yaptım?" | `brain recap` özeti |
| "beyni kontrol et" | Önce bir lint raporu, düzeltmeler ancak senin onayınla |

`Core.md` değişiklikleri her zaman senin onayını ister. Geri kalan her şey sormadan
kaydedilir ve her yazım vault'unda bir git commit'i olur. Claude asla push yapmaz; o sana
kalmış.

CLI'ı elle de kullanabilirsin; yolu `vault/.brain/bin/brain`:

```
brain today | journal | todo [--soon] | new <tür> "<Başlık>" | log <işlem> "<Başlık>"
brain search [--raw] <terim> | recap [--days N] | lint-check
```

## Diller

Sistemin kendi metinleri İngilizce: şema, skill'ler, başlıklar, kayıt türleri, frontmatter.
Not içeriği kaynağına göre yazılır: bir makale kendi dilinde özetlenir, sohbet notları ise
konuştuğun dilde yazılır. Her sayfa dilini `lang:` alanında tutar. Arama Türkçe `İ/ı` ile
`I/i` harflerini aynı sayar.

## Ayarlar

- **Kişisel kurallar:** Claude'a söyle ("bundan sonra …"); onayınla vault'undaki `CLAUDE.md` dosyasının *Personal rules* bölümüne ekler.
- **Özel sayfalar:** `private: true` olan bir sayfa oturum başı bağlamına girmez.
- **Ortam değişkenleri**, `~/.claude/settings.json` içinde `env` altına yazılır:
  - `BRAIN_STOP_CHECK=0` Stop hook'unu kapatır.
  - `BRAIN_STOP_MIN_TOOLS=10` kaç araç çağrısının "gerçek iş" sayılacağını belirler.
  - `BRAIN_CONTEXT_MAX=12000` oturum başı bağlamın üst sınırını bayt olarak belirler.

## Güncelleme ve kaldırma

```bash
git -C ~/Projects/second-brain pull
```

```bash
~/Projects/second-brain/install.sh
```

Vault bu klasöre bağlı olduğu için hook'lar, skill'ler ve şema `git pull` ile güncellenir.
Kurulum betiğini tekrar çalıştırmak yalnızca betiğin kendisi değiştiyse gerekir.

```bash
~/Projects/second-brain/install.sh --uninstall
```

Bu komut skill'leri, `~/.claude/CLAUDE.md` bölümünü ve ayar girdilerini kaldırır. Vault'un
olduğu gibi kalır.

## Gizlilik

Her şey bilgisayarında ve kendi git deponda kalır. Şema sırların (şifre, anahtar, token,
kimlik ya da kart numarası) yazılmasını yasaklar. Vault'unu push edeceksen özel bir repo
kullan.

## Teşekkür

- Andrej Karpathy'nin [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)'si: raw / wiki / şema katmanları ve ingest, query, lint işlemleri.
- [avenox Beyin](https://github.com/avenoxai/avenoxbeyin): çekirdek bir profil ve günlük kayıtlarla Claude'un tuttuğu kişisel hafıza fikri.

## Lisans

[GPL-3.0](LICENSE)
