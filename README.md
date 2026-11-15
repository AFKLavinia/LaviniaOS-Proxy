# LaviniaOS Proxy (`lavinia-client`)

**LaviniaOS Proxy**, gecikmeye duyarlı uygulamalar, rekabetçi oyunlar ve yüksek bant genişliği gerektiren iş yükleri için tasarlanmış, **Sıfır Kayıplı Saydam Proxy (Zero-Loss Transparent Proxy)** ve **Aktif Kuyruk Yönetimi (AQM / Debloat)** masaüstü istemcisidir.

Rust (`tokio`), C11 (`liblavinia_tun`) ve Tauri v2 (`React 18 + TypeScript`) ile geliştirilen istemci; sistem genelindeki TCP, UDP ve DNS trafiğini çekirdek seviyesinde yakalar, tampon şişmesini (bufferbloat) önler ve SOCKS5 tünelleri üzerinden güvenli, düşük gecikmeli olarak yönlendirir.

---

## Temel Özellikler / Key Features

- **Zero-Arc Saydam TCP Yönlendirici (`tcp.rs`)**:
  - `tokio::net::TcpStream::into_split` ve özel 16 KB yığın tamponları ile çalışma zamanında sıfır `Arc` tahsisi ve `TCP_NOTSENT_LOWAT` senkronizasyonu.
  - Lock-free atomik zaman damgaları ile kilitlenmesiz boşta kalma (idle timeout) takibi.
- **TPROXY Saydam UDP & Spoof Önbelleği (`udp.rs`)**:
  - Linux `IP_TRANSPARENT` / `IP_RECVORIGDSTADDR` ile gerçek hedef adresini koruyan saydam UDP yönlendirme.
  - STUN, WebRTC ve oyun trafiğinde sistem çağrısı (syscall) yükünü ortadan kaldıran oturum bazlı 32-girişli `spoof_cache`.
- **RFC 8289 Adaptive CoDel & DRR Kuyruk Yönetimi (`aqm_controller.rs`, `aqm.rs`)**:
  - 100 ms kayan pencere üzerinde minimum bekleme süresi (sojourn time) analizi ile tampon şişmesini (bufferbloat) anında tespit edip kontrol altına alan aktif kuyruk yöneticisi.
  - Derin Paket İnceleme (`classifier.rs`) ile DNS, VoIP, Oyun ve Yığın (Bulk) trafiğini ayrıştıran Deficit Round Robin (DRR) planlayıcı.
- **Sıfır Tahsisli Oyun Algılayıcı (`game_detector.rs`)**:
  - `/proc` dosya sistemini yığın (stack) tamponlarıyla sıfır heap tahsisiyle tarayan, bilinen oyun/ses süreçlerini $O(1)$ PID önbelleğiyle izleyen ve boşta 10 saniye adaptif uyku moduna geçen akıllı algılayıcı.
- **Yerel Do53 Saydam DNS & Adblock (`dns.rs`)**:
  - EDNS0 (RFC 6891) uyumlu yerel DNS vekili, 4096-girişli önbellek ve gömülü reklam/izleyici engelleme motoru.
- **Sanal Ethernet (`lavinia0`) Tam Sistem Yönlendirmesi (`firewall.rs`, `lavinia-net-helper`)**:
  - Linux üzerinde `lavinia0` (`10.200.0.1/30`) <-> `lavinia0-gw` (`10.200.0.2/30`) sanal Ethernet (`veth`, MTU `1420`) çifti ve politika yönlendirme tablosu (`table 4178`) oluşturularak tüm sistem trafiği özel sanal ağ arayüzü üzerinden geçirilir.
  - Upstream proxy hedefi (`priority 50`), `SO_MARK` (`0x4c42`, `priority 55`) işaretli motor soketleri ve yerel ağ blokları (`priority 60`) `main` yönlendirme tablosundan doğrudan fiziksel arayüze (`wlan0`/`usb0`/`eth0`) yönlendirilerek yönlendirme döngüleri (routing loop) sıfırlanır.
- **Fail-Closed Killswitch & Polkit Güvenlik Modeli (`lavinia-net-helper`)**:
  - En az yetki prensibiyle çalışan istemci; ağ kurallarını (`nftables`) pozisyonel argümanlarla korunan ayrıcalıklı yardımcı betik üzerinden uygular.
  - İstemci beklenmedik şekilde sonlansa (`SIGKILL`) dahi arka plandaki watchdog sızıntıyı önlemek için `lavinia0` sanal arayüzünü temizler ve `inet lavinia_killswitch` tablosunu devreye sokar.
- **60 FPS GPU Hızlandırmalı Telemetri Arayüzü (`Stats.tsx`)**:
  - Catmull-Rom kübik Bezier eğrileriyle çizilen gerçek zamanlı indirme/yükleme hızı, RTT ve paket istatistikleri; Default, Midnight ve OLED tema desteği.

---

## Mimari Genel Bakış / Architecture Overview

Proje dört ana katmandan oluşur (detaylı modül şeması için bkz. [`maps.md`](./maps.md), optimizasyon geçmişi için bkz. [`log.md`](./log.md)):

1. **Ön Yüz (`src/ui/`)**: React 18, TypeScript ve Vite ile geliştirilmiş, Tauri v2 IPC köprüsü üzerinden arka uçla haberleşen masaüstü arayüzü.
2. **Çekirdek Ağ Motoru (`src-tauri/src/engine/`)**: Saydam TCP/UDP/DNS yönlendirme, SOCKS5 el sıkışması, CoDel AQM, DPI sınıflandırıcı, çift hat (Bicast) yedekleme ve oyun algılama modülleri.
3. **Platform Entegrasyonu (`src-tauri/src/platform/` & `deploy/`)**: Linux (`nftables`, `cgroups v2` split-tunneling, Polkit, NetworkManager) ve Windows (`WFP`, `WinTUN`) işletim sistemi adaptörleri.
4. **C11 Veri Düzlemi (`src/c/`)**: Lock-free SPSC halka tampon (`ring_buffer.c`) ve düşük seviyeli platform TUN sürücüleri (`tun_linux.c`, `tun_windows.c`).

---

## Gereksinimler / Requirements

### Linux (Arch / Debian / Ubuntu / Fedora)
- **Rust**: `1.77+` (`rustup` üzerinden)
- **C Derleyicisi**: `gcc` veya `clang` (C11 destekli)
- **Node.js**: `20+` ve `npm`
- **Sistem Kütüphaneleri**:
  - Arch Linux: `gtk3 webkit2gtk-4.1 nftables polkit iproute2 libnotify`
  - Ubuntu / Debian: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf nftables polkitd iproute2`

### Windows
- **Rust**: `1.77+` (MSVC toolchain)
- **Visual Studio Build Tools**: C/C++ masaüstü geliştirme iş yükü
- **Node.js**: `20+`

---

## Geliştirme ve Çalıştırma / Development

Projeyi geliştirme modunda tek komutla başlatmak için kök dizindeki `start.sh` betiğini kullanabilirsiniz:

```bash
./start.sh
```

`start.sh` (veya `scripts/dev-runner.sh`), eksik UI bağımlılıklarını kontrol eder, Linux üzerinde kesintisiz UDP TPROXY testi için isteğe bağlı `cap_net_admin+ep` yetkisini tanımlar ve Tauri geliştirme sunucusunu başlatır.

### Manuel Komutlar

```bash
# 1. Ön yüz bağımlılıklarını yükleyin ve derleyin
npm --prefix src/ui install
npm --prefix src/ui run build

# 2. Rust ve C11 çekirdek birim testlerini çalıştırın
cargo test --lib

# 3. Üretim (Release) paketini derleyin
cargo build --release
```

---

## Paketleme ve Kurulum / Packaging

### Arch Linux (`PKGBUILD`)

```bash
cd deploy/arch
makepkg -si
```

Kurulum paketi aşağıdaki bileşenleri sisteminize yerleştirir:
- `/usr/bin/lavinia-client` (`cap_net_admin+ep` yetkisi `lavinia-client.install` kancasıyla otomatik tanımlanır)
- `/usr/lib/lavinia/lavinia-net-helper` (Ayrıcalıklı `nftables` ve killswitch watchdog yardımcısı)
- `/usr/share/polkit-1/actions/os.lavinia.client.policy` (Polkit yetki politikası)
- `/etc/lavinia/lavinia-client.conf` (Paket yapılandırması)
- `/usr/share/applications/lavinia-client.desktop` & `/usr/share/pixmaps/lavinia-client.png`

---

## Lisans / License

Bu proje [MIT Lisansı](./LICENSE) altında lisanslanmıştır.
