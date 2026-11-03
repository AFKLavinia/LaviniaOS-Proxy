# LaviniaOS Proxy - Sistem Optimizasyon, Güvenlik ve Mimari Değişiklik Günlüğü (log.md)

**Tarih:** 2026-11-03  
**Sürüm:** v0.2.0-hardened  
**Donanım / Çalışma Ortamı Tespiti:** 4 CPU Çekirdeği (Quad-Core x86_64 Linux), Node.js v26.13.0, Rust 1.99.1  

---

## 1. Optimal Alt Ajan Dağılımı ve Yönetim Mimarisi

Sistem çekirdek sayısı (`nproc` = 4) analiz edilerek sistem kaynaklarını tüketmeden, kilitlenmelere ve CPU darboğazına yol açmadan en yüksek verimlilik sağlayacak **3 uzman alt ajan** ile paralel çalışma yürütülmüştür:
1. **Network Core & AQM Architect (`f52e588f-fa39-4846-9b29-e1448d7fd13e`)**: TCP/UDP soket katmanı, RFC 8289 CoDel buffer debloat algoritması ve sıfır tahsisli soket mimarisi.
2. **Security & Kernel Architect (`0ecdff4a-7809-4c3d-bc07-4ccb0be405bc`)**: Linux nftables çekirdek hizalaması, fail-closed killswitch, cgroups v2 split-tunneling ve düşük güç tüketimli `/proc` izleme.
3. **Frontend UI/UX Specialist (`31baa2f2-9a34-4191-85d8-3b542285d8b9`)**: 60 FPS Catmull-Rom cubic Bezier telemetri grafikleri, tema SVG gradyanları, akıllı yapıştırma ayrıştırıcısı ve erişilebilirlik.

---

## 2. Buffer Debloat ve AQM Mimarisi (RFC 8289 Adaptive CoDel)

### 2.1. Eski Sistemdeki Eksiklikler
- Sabit eşik değerli (90ms/140ms) gecikme kontrolleri hücresel ağlardaki (LTE/5G) doğal dalgalanmalarda (HARQ yeniden iletimleri, DRX uyku döngüleri) paket fırtınasıyla seri drop yaratıyordu.
- Matematiksel bir ters karekök kontrol yasası bulunmadığı için tampon boşaltması TCP tıkanıklık denetimiyle (BBR, Cubic) senkronize olamıyordu.

### 2.2. Yeni RFC 8289 Adaptive CoDel Uygulaması (`aqm_controller.rs`)
- **100ms Sliding Window Min Sojourn Tracker (`MinSojournTracker`)**:
  - Gecikme anlık bir tepe noktasına göre değil, 100ms kayan pencere içindeki *minimum bekletilme (sojourn)* süresine göre değerlendirilir.
  - Atomik `compare_exchange_weak` ile lock-free olarak güncellenir.
- **Adaptif Hedef (Target Delay)**:
  - Standart mod: `10ms` (10,000 µs).
  - Oyun / Düşük Gecikme modu: `15ms` (15,000 µs) hücresel jitter payı.
- **Ters Karekök Kontrol Yasası (Inverse Square-Root Law)**:
  $$\Delta t = \frac{\text{interval}}{\sqrt{\text{count}}}$$
  $$t_{\text{next}} = t_{\text{scheduled}} + \max\left(\Delta t, 2000\,\mu\text{s}\right)$$
  Tampon sürekli dolu kaldığında drop aralığı $1/\sqrt{n}$ oranında sıklaşır; tampon normale indiğinde dropping durumu anında terk edilir.
- **Kontrol Paketleri Koruması**:
  - DNS, ACK ve SYN gibi kontrol paketleri (`is_control`) asla düşürülmez.

---

## 3. Ağ & Bellek Optimizasyonları (TCP & UDP Relay)

### 3.1. TCP Relay (`src-tauri/src/engine/tcp.rs`)
- **16KB Tampon Hizalaması**:
  - Kullanıcı alanındaki `RELAY_BUF` 64KB'den 16KB'ye düşürüldü.
  - Kernel düzeyindeki `TCP_NOTSENT_LOWAT` (16,384 bayt) ile tam hizalandı.
  - 2,048 eşzamanlı bağlantıda kullanıcı alanında ~192 MB RAM tasarrufu sağlandı ve soket kuyruğunda bekleyen paket gecikmesi minimize edildi.
- **Sıfır Arc Tahsisi (`split()` vs `into_split()`)**:
  - `into_split()`'in yarattığı dahili `Arc<OwnedReadHalf>` ve `Arc<OwnedWriteHalf>` kaldırıldı. Referans bazlı `split()` ile sıfır heap tahsisli okuma/yazma döngüsüne geçildi.
- **Lock-Free Aktivite Takibi ve Kanal Tasarrufu**:
  - Her TCP paketinde `mpsc::channel` üzerinden mesaj gönderip timer resetleme döngüsü kaldırıldı.
  - `AtomicU64` ile mikro-saniye hassasiyetinde lock-free zaman damgası tutularak 15 saniyede bir hafif kontrolle 60 saniyelik idle timeout sağlandı. Bu sayede paket başına timer/kanal işletim sistemi maliyeti sıfırlandı.

### 3.2. UDP Relay (`src-tauri/src/engine/udp.rs`)
- **32-Girişli Sınırlandırılmış Spoof Soket Önbelleği (`spoof_cache`)**:
  - STUN ve P2P oyun protokollerinde kaynak ve hedef portu farklı yanıtlar geldiğinde her paket için çağrılan `platform::udp_spoof_socket()` (`socket()`, `setsockopt(IP_TRANSPARENT)`, `bind()`, `close()`) syscall fırtınası engellendi.
  - Oturum başına en fazla 32 adet spoof soket `HashMap` içinde saklanarak tekrarlayan datagramlarda syscall maliyeti sıfıra indirildi.

---

## 4. Düşük Güç Tüketimi (CPU & Pil Tasarrufu)

### 4.1. İşlem İzleme (`src-tauri/src/engine/game_detector.rs`)
- **Önbelleklenmiş PID $O(1)$ Hızlı Yolu**:
  - Oyun aktifken her 3 saniyede tüm `/proc` dizinini taramak yerine, tespit edilen PID önbelleğe alınır ve sadece `/proc/<pid>/comm` doğrulanarak $O(1)$ sürede kontrol edilir.
- **Sıfır Heap Tahsisi (Zero-Alloc)**:
  - String ve PathBuf tahsisleri kaldırıldı. Yığın üzerinde `[u8; 64]` ve `[u8; 512]` sabit tamponlar ve in-place dilimleme (`trim_ascii`, `inspect_cmdline`) kullanıldı.
  - Wine/Proton ve Steam (`pressure-vessel`) argümanları bellek tahsissiz doğrudan bayt seviyesinde ayrıştırılır.
- **Adaptif Uyku (Adaptive Sleep)**:
  - Sistem boştayken tarama aralığı 3 saniyeden 10 saniyeye çıkarıldı (CPU timer uyandırmalarında %70 tasarruf). Oyun çalışırken tepkiselliği korumak için 3 saniye aralık kullanılır.

---

## 5. Linux Kernel, Routing & Güvenlik Hardening

### 5.1. Privileged Helper (`deploy/polkit/lavinia-net-helper`)
- **Fail-Closed Killswitch**:
  - İstemci normal kapandığında kurallar temizlenir.
  - İstemci beklenmedik şekilde çökerse (SIGKILL, segfault), arka plandaki watchdog unencrypted internet sızıntısını önlemek için `inet lavinia_killswitch` tablosunu derhal devreye alarak LAN dışındaki tüm trafiği engeller (`policy drop`).
- **Komut Enjeksiyonu Önleme**:
  - `bash -c` içine parametre gömme kaldırıldı. Argümanlar `execve` tipi `$1`, `$2` pozisyonel parametreleriyle aktarıldı.
  - Sıkı parametre regex doğrulamaları (`validate_ip`, `validate_port`, `validate_session_id`, `validate_pid`, `validate_killswitch`).
- **Yetki Yükseltme Koruması**:
  - `grant-capabilities` hedefi kesin olarak `/usr/bin/lavinia-client` veya `/usr/local/bin/lavinia-client` olmalı, `root:root` sahipliğinde olmalı ve grup/diğer kullanıcılara yazma izni bulunmamalıdır (`chmod 0755`).
- **Watchdog Uyku Optimizasyonu**:
  - 100ms'lik döngü 500ms'ye (`sleep 0.5`) çıkarılarak CPU bağlam değişimi %80 azaltıldı.

### 5.2. Kernel nftables Hizalaması (`src-tauri/src/platform/linux/firewall.rs`)
- **Dinamik PMTU/MSS Clamping**:
  - Sabit `1400` değeri yerine `tcp flags syn tcp option maxseg size set rt mtu` kuralına geçildi. Kernel, o anki yönlendirme MTU'suna göre MSS'i dinamik olarak belirler.
- **Loopback Dokunulmazlığı**:
  - `oifname "lo" return` ve `fib daddr type { local, broadcast, multicast } return` kuralları MSS kuralının önüne alınarak yerel IPC ve loopback trafiğinde 64KB MTU korunmuştur.
- **Cgroups v2 Split-Tunneling Entegrasyonu**:
  - `/sys/fs/cgroup/lavinia_bypass` cgroup dizini otomatik yetkilendirilerek proxy'yi baypas etmek isteyen süreçler için `socket cgroupv2 level 1 "lavinia_bypass" return` kuralları yerleştirildi.

---

## 6. Frontend UI/UX, Telemetri ve Ergonomi

### 6.1. 60 FPS Catmull-Rom Bezier Spline Telemetrisi (`Stats.tsx`)
- Eski sert köşeli `polyline` yerine Catmull-Rom kübik Bezier eğri algoritması entegre edildi.
- İndirme ve yükleme grafikleri için tema duyarlı SVG dikey gradyanları (`<linearGradient>`) eklendi (Midnight, OLED ve Standart koyu tema).
- `React.memo` ile telemetri güncellemeleri form girişlerinden izole edildi.

### 6.2. Akıllı URL ve Format Ayrıştırıcısı (`TargetForm.tsx`)
- `parseEndpoint` fonksiyonu:
  - `http://192.168.43.1:1080`
  - `socks5://user:pass@192.168.43.1:1080`
  - `tg://socks?server=192.168.43.1&port=1080&user=...&pass=...`
  - `[2001:db8::1]:1080` (IPv6)
  - `192.168.43.1:1080`
- Yapıştırma anında port aralığı otomatik olarak `1..65535` arasına kırpılır (clamp) ve tespit edilen kullanıcı adı/şifre otomatik kaydedilir.

### 6.3. Modal Seçim Güvenliği (`SettingsModal.tsx`, `AuthModal.tsx`, `PermissionModal.tsx`)
- Kullanıcı metin seçerken mouse'u dışarı taşıdığında modalın kapanması `onMouseDown={(e) => { if (e.target === e.currentTarget) ... }}` kontrolü ile engellendi.

---

## 7. Doğrulama ve Test Sonuçları

- **Cargo Test:** `~/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml`
  - **Sonuç:** `11 passed; 0 failed; 0 ignored; finished in 0.01s` (Tüm birim ve entegrasyon testleri başarıyla geçti).
- **TypeScript Derleme:** `PATH="/home/alper/.nvm/versions/node/v26.13.0/bin:$PATH" npm run --prefix src/ui build`
  - **Sonuç:** `tsc --noEmit && vite build` 0 hata ile 2.80s içinde tamamlandı.
- **Kullanıcı Kısıtlaması Uyumu:** `cargo build --release` ve `npx tauri build` komutları çalıştırılmamış, kurala %100 riayet edilmiştir.
