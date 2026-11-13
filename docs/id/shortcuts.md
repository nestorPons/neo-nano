# Pintasan papan ketik

nnano memakai **peta tombol bergaya desktop** secara default. Pintasan nano yang tidak bertabrakan tetap dipertahankan.

**Notasi:** `Ctrl` = `^` · `Alt` = `M-`. Di macOS, `Alt` adalah `Option`; `Cmd+*` tidak pernah sampai ke aplikasi.
Kolom *Alternatif* adalah pintasan warisan nano, bila ada.

## Berkas

| Tindakan | Tombol | Alternatif |
|---|---|---|
| Simpan | `Ctrl+S` | — |
| Simpan sebagai | `Alt+S` | — |
| Buka berkas | `Ctrl+O` | — |
| Muat ulang dari disk | `Ctrl+R` | — |
| Sisipkan berkas di kursor | `Alt+I` | `Ctrl+R` (nano) |
| Buffer baru | `Ctrl+N` | — |
| Tab baru | `Ctrl+T` | — |
| Tutup buffer | `Ctrl+W` | — |
| Keluar | `Ctrl+Q` | — |

## Penyuntingan

| Tindakan | Tombol | Alternatif |
|---|---|---|
| Urungkan | `Ctrl+Z` | `Alt+U` |
| Ulangi | `Ctrl+Y` | `Alt+E` |
| Potong | `Ctrl+X` | — |
| Salin | `Ctrl+C` | — |
| Tempel | `Ctrl+V` | `Ctrl+U` |
| Potong baris | `Ctrl+K` | — |
| Pilih semua | `Ctrl+A` | — |
| Mulai tanda | `Alt+A` | `Alt+A` |
| Salin teks bertanda | `Alt+6` | `Alt+6` |
| Tanda persegi | `Alt+Z` | — |
| Hapus karakter | `Ctrl+D` | — |
| Ganti | `Ctrl+\` | `Alt+R` |
| Ratakan paragraf | `Ctrl+J` | — |
| Jalankan perintah luar | `Alt+X` | — |

## Gerak

| Tindakan | Tombol | Alternatif |
|---|---|---|
| Ke baris | `Ctrl+G` | `Alt+G` |
| Awal baris | `Home` | — |
| Akhir baris | `End` | `Ctrl+E` |
| Baris pertama | `Alt+\` | `Alt+\` |
| Baris terakhir | `Alt+/` | `Alt+/` |
| Satu kata ke kiri / kanan | `Ctrl+←` / `Ctrl+→` | `Alt+←` / `Alt+→` |
| Perluas pilihan per karakter | `Shift+←` / `Shift+→` | — |
| Perluas pilihan per baris | `Shift+↑` / `Shift+↓` | — |
| Satu halaman ke atas | `PgUp` | `Alt+↑` |
| Satu halaman ke bawah | `PgDn` | `Alt+↓` |
| Satu karakter mundur | `Ctrl+B` | `Ctrl+B` |

`Shift+←`/`Shift+→` memperluas tanda per karakter dan `Shift+↑`/`Shift+↓` per baris sambil mempertahankan kolom visual, seperti di editor desktop; `Esc` menghapusnya. Saat tanda aktif, setiap gerakan memperluasnya, seperti di nano. `Ctrl+C` menyalin lalu menghapus tanda, jadi `Ctrl+V` berikutnya menyisipkan di kursor.

## Pencarian

| Tindakan | Tombol | Alternatif |
|---|---|---|
| Cari | `Ctrl+F` | — |
| Cari berikutnya | `F3` | `Alt+W` |
| Cari sebelumnya | `Shift+F3` | `Alt+B` |
| Peka huruf besar/kecil (di prompt) | `Alt+C` | `Alt+C` |
| Ekspresi reguler (di prompt) | `Alt+R` | `Alt+R` |

Pencarian meniru nano: **tidak inkremental** (berjalan saat prompt diterima), literal secara default, tidak peka huruf besar/kecil secara default, dan berputar ke awal buffer dengan pesan *pencarian diulang dari awal*.
`Alt+R` bersifat modal: di dalam prompt mengganti mode ekspresi reguler, di luarnya membuka Ganti.

## Antarmuka

| Tindakan | Tombol | Alternatif |
|---|---|---|
| Bantuan | `Ctrl+H` | `F1` |
| Menu pengaturan | `Alt+,` | — |
| Palet perintah | `Ctrl+P` | — |
| Ubah penyorotan sintaksis | `Alt+Y` | `Alt+Y` |
| Gambar ulang layar | `Ctrl+L` | `Ctrl+L` |
| Bahasa antarmuka | `Alt+,` → Bahasa | `--language <kode>` |

Palet (`Ctrl+P`) menampilkan tombol setiap perintah di kanan, dan mengetik sebuah tombol (`ctrl+q`) menyaring menurut pintasannya. Bantuan (`Ctrl+H`) dan palet dihasilkan dari tabel tombol yang sama dengan yang dibaca dekoder, jadi keduanya tidak mungkin menampilkan tombol yang tidak berfungsi.

Antarmuka berbicara sepuluh bahasa: English, 简体中文, हिन्दी, Español, العربية, Français, বাংলা, Português, Bahasa Indonesia, dan 日本語. Pilih di pengaturan (`Alt+,` → Bahasa) atau dengan `--language id`; `language = "auto"` di `config.toml` mengikuti `$LC_ALL`/`$LANG`.

## Yang berubah dari nano

| Sebelumnya (nano) | Sekarang |
|---|---|
| `Ctrl+O` simpan | `Ctrl+S` menyimpan |
| `Ctrl+X` keluar | `Ctrl+Q` keluar · `Ctrl+X` memotong |
| `Ctrl+W` cari | `Ctrl+F` mencari · `Ctrl+W` menutup buffer |
| `Ctrl+C` posisi kursor | Baris status · `Ctrl+C` menyalin |
| `Ctrl+V` halaman berikutnya | `PgDn` / `Alt+↓` · `Ctrl+V` menempel |
| `Ctrl+Y` halaman sebelumnya | `PgUp` / `Alt+↑` · `Ctrl+Y` mengulangi |
| `Ctrl+A` awal baris | `Home` · `Ctrl+A` memilih semua |
| `Ctrl+Z` menangguhkan | Dihapus · `Ctrl+Z` mengurungkan |
| `Ctrl+R` menyisipkan berkas | `Alt+I` · `Ctrl+R` memuat ulang |
| `Ctrl+N` / `Ctrl+P` baris berikut / sebelumnya | Panah `↓` / `↑` · buffer baru / palet |
| `Ctrl+T` eksekusi | `Alt+X` · `Ctrl+T` membuka tab |
| `Ctrl+G` bantuan | `F1` · `Ctrl+G` ke baris |

Tidak berubah: `Ctrl+K`, `Ctrl+U`, `Ctrl+E`, `Ctrl+D`, `Ctrl+J`, `Ctrl+L`, `Ctrl+B`, `Ctrl+\`, `Alt+U`, `Alt+E`, `Alt+A`, `Alt+6`, `Alt+G`, `Alt+\`, `Alt+/`, `Alt+W`, `Alt+B`, `Alt+C`, `Alt+R`, `Alt+Y`.

## Batas terminal

- `Ctrl+S` dan `Ctrl+Q` memerlukan kendali aliran XON/XOFF dimatikan; bila tidak, terminal menelannya.
- `Enter`, `Tab`, `Backspace`, dan `Esc` tidak dapat dipetakan ulang.
- `Ctrl+B` adalah awalan bawaan tmux; `Ctrl+A`, milik screen.
- `Ctrl+Shift+<huruf>` tidak andal di terminal lama: tidak ada `Ctrl+Shift+Z` untuk mengulangi.
- Untuk memilih teks dengan terminal, tahan `Shift` saat menyeret.
- nnano memakai mouse bila opsi `mouse` aktif (bawaan): klik meletakkan kursor, seret menandai, `Alt`+seret menandai persegi, roda menggulir tanpa memindahkan kursor, dan mengklik bilah gulir di kanan melompat ke bagian berkas itu. Melepas tombol menyalin teks yang ditandai ke register internal dan ke papan klip sistem (OSC 52); tanda tetap terlihat sampai Anda mengklik di tempat lain atau menekan `Esc`.
- Bilah status menampilkan `baris:kolom/total`: baris kursor, kolom visualnya, dan jumlah total baris dokumen.
- Grup urungkan: pengetikan dikelompokkan per kata dan per jeda 400 ms; potong, tempel, simpan, dan pindah buffer menutup grup. "Ganti semua" adalah satu langkah.
- `Ctrl+H` membuka referensi pintasan. Backspace adalah tombol Backspace terminal (`0x7F`): bila terminal Anda mengirim `0x08`, tombol itu membuka bantuan — atur agar mengirim `0x7F`.

## Prompt dan berkas pemulihan

- `Alt+S` meminta jalur, dan `Ctrl+S` juga pada buffer tanpa nama (`nnano -`, buffer baru, atau "simpan sebagai" yang dibatalkan). `Tab` melengkapi jalur di *Simpan sebagai*, *Buka*, dan *Sisipkan berkas*; satu kandidat mengisi baris, beberapa memperpanjangnya sampai awalan bersama.
- `Alt+S` menulis berkas baru dan memakai namanya: bilah buffer, bilah status, dan tata bahasa penyorotan mengikutinya. Penulisan yang gagal membiarkan nama lama.
- `Alt+X` menjalankan baris dengan `sh -c` dan menyisipkan keluarannya di kursor sebagai satu langkah urungkan; bilah status melaporkan kode keluar dan baris pertama galat.
- `Ctrl+R` membaca ulang berkas buffer ini dari disk dan mengganti teksnya; riwayat urungkan teks lama dibuang. Jika ada perubahan belum disimpan, ia bertanya sekali (`Ctrl+R` lagi memuat ulang, `Ctrl+S` menyimpan).
- `Alt+I` meminta berkas lalu menyisipkan teksnya di kursor sebagai satu langkah urungkan; `Tab` melengkapi jalurnya. Buffer tetap memakai nama dan pengodeannya sendiri.
- Selama buffer punya perubahan belum disimpan, nnano menulis cuplikan pemulihan 2 detik setelah suntingan terakhir di `$XDG_STATE_HOME/nnano/recovery/`. Kecelakaan atau sesi SSH yang terbunuh dipulihkan otomatis saat mulai berikutnya (`recovered unsaved changes for …`); menyimpan atau keluar bersih menghapusnya.
- `Ctrl+C` dan `Alt+6` selalu menyalin ke register internal. Aktifkan opsi `osc52` (`--osc52`) untuk juga mengirim salinan ke papan klip sistem, maksimum 64 KiB; pemotongan dilaporkan di bilah status. Baris status melaporkan ukuran yang disalin, dan `Ctrl+V` pada register kosong memberi tahu.
- `Ctrl+J` membungkus ulang paragraf di sekitar kursor (atau baris bertanda) selebar area teks, mempertahankan indentasi baris pertama. Ini satu langkah urungkan.
