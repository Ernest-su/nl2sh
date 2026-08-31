TERMUX_PKG_HOMEPAGE=https://github.com/Ernest-su/nl2sh
TERMUX_PKG_DESCRIPTION="Android-first natural-language shell agent with a local safety boundary"
TERMUX_PKG_LICENSE="MIT"
TERMUX_PKG_LICENSE_FILE="LICENSE"
TERMUX_PKG_MAINTAINER="@Ernest-su"
TERMUX_PKG_VERSION="1.0.0"
TERMUX_PKG_SRCURL="https://github.com/Ernest-su/nl2sh/archive/refs/tags/v${TERMUX_PKG_VERSION}.tar.gz"
TERMUX_PKG_SHA256=6bcbfc05601542578239a51bc2f8a52efda3848f7f35616b5bbc9f95caf60402
TERMUX_PKG_API_LEVEL=26
TERMUX_PKG_BUILD_IN_SRC=true
TERMUX_PKG_AUTO_UPDATE=true

termux_step_pre_configure() {
	termux_setup_rust
}

termux_step_make() {
	cargo build \
		--locked \
		--no-default-features \
		--jobs "${TERMUX_PKG_MAKE_PROCESSES}" \
		--target "${CARGO_TARGET_NAME}" \
		--release
}

termux_step_make_install() {
	install -Dm755 \
		"target/${CARGO_TARGET_NAME}/release/nl2sh" \
		"${TERMUX_PREFIX}/bin/nl2sh"
	install -Dm644 config.toml.example \
		"${TERMUX_PREFIX}/share/nl2sh/config.toml.example"
	install -Dm644 README.md \
		"${TERMUX_PREFIX}/share/doc/nl2sh/README.md"
}
