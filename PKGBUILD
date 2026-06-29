pkgname=s3-tui
pkgver=0.1.0
pkgrel=1
pkgdesc="A terminal user interface for managing buckets and objects in S3 compatible storage"
arch=('x86_64')
url="https://github.com/Seriklol/s3-tui"
license=('MIT')
depends=('gcc-libs' 'glibc')
makedepends=('cargo')
options=(!lto)

build() {
  cd "$startdir"
  cargo build --release
}

package() {
  cd "$startdir"
  install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
