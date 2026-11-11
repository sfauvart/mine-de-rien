# Image de l'agent : le binaire `mine-de-rien`, datapack compris, et le plugin Paper
# `mine-de-rien-voix.jar` que l'agent livre au serveur. Rien d'autre.
#
# Le binaire est statique (musl) et compilé sur la plateforme de la machine qui construit,
# pour amd64 comme pour arm64 : pas d'émulation QEMU pendant la compilation, qui prendrait
# une heure. Les versions de base sont figées : deux constructions du même commit donnent la
# même image.

# --- Construction, sur la plateforme de la machine (BUILDPLATFORM) ---------------------------
FROM --platform=$BUILDPLATFORM rust:1.94.0-slim-trixie AS construction

# SQLite et la cryptographie de TLS sont en C : clang les compile pour la cible, avec les
# en-têtes musl de Debian pour chaque architecture. L'édition de liens est faite par le
# `rust-lld` livré avec Rust, sans compilateur croisé.
RUN dpkg --add-architecture amd64 \
 && dpkg --add-architecture arm64 \
 && apt-get update \
 && apt-get install -y --no-install-recommends clang llvm musl-dev:amd64 musl-dev:arm64 \
 && rm -rf /var/lib/apt/lists/* \
 && rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

ENV CC_x86_64_unknown_linux_musl=clang \
    AR_x86_64_unknown_linux_musl=llvm-ar \
    CFLAGS_x86_64_unknown_linux_musl="--target=x86_64-unknown-linux-musl -nostdlibinc -isystem /usr/include/x86_64-linux-musl" \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
    CC_aarch64_unknown_linux_musl=clang \
    AR_aarch64_unknown_linux_musl=llvm-ar \
    CFLAGS_aarch64_unknown_linux_musl="--target=aarch64-unknown-linux-musl -nostdlibinc -isystem /usr/include/aarch64-linux-musl" \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld

ARG TARGETARCH
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-musl > /cible ;; \
      arm64) echo aarch64-unknown-linux-musl > /cible ;; \
      *) echo "architecture non prise en charge : $TARGETARCH" >&2 ; exit 1 ;; \
    esac

WORKDIR /src
# Les dépendances d'abord, sur un crate vide : cette couche ne se refait que si elles changent.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
 && echo 'fn main() {}' > src/main.rs \
 && : > src/lib.rs \
 && cargo build --release --locked --target "$(cat /cible)" \
 && rm -rf src

COPY src ./src
# Embarqué dans le binaire à la compilation : `mine-de-rien datapack` le recopie tel quel.
COPY datapacks ./datapacks
RUN touch src/main.rs src/lib.rs \
 && cargo build --release --locked --target "$(cat /cible)" \
 && cp "target/$(cat /cible)/release/mine-de-rien" /mine-de-rien

# --- Le plugin Paper, construit une fois : un .jar est le même sur toutes les architectures --
FROM --platform=$BUILDPLATFORM maven:3.9.16-eclipse-temurin-25 AS plugin
WORKDIR /plugin
COPY plugin/pom.xml ./
COPY plugin/src ./src
# Les dépendances (API de Paper et du chat vocal, JUnit) restent dans un cache de
# construction : aucune n'entre dans le .jar. Les tests du plugin tournent ici : une image ne
# se construit pas sur un plugin qui échoue.
RUN --mount=type=cache,target=/root/.m2 mvn -B -q package

# --- Image finale : le binaire et le plugin, rien d'autre ------------------------------------
FROM scratch
COPY --from=construction /mine-de-rien /usr/local/bin/mine-de-rien
# Recopié chez le serveur par « mine-de-rien livrer », avant son démarrage.
COPY --from=plugin /plugin/target/mine-de-rien-voix.jar /usr/local/share/mine-de-rien/mine-de-rien-voix.jar
ENV PATH=/usr/local/bin
WORKDIR /app

ARG VERSION=dev
ARG REVISION=inconnue
LABEL org.opencontainers.image.title="mine-de-rien" \
      org.opencontainers.image.description="Agent Mine de rien, son datapack et son plugin Paper : les devoirs de CM2 mis en jeu dans Minecraft." \
      org.opencontainers.image.source="https://github.com/sfauvart/mine-de-rien" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="$VERSION" \
      org.opencontainers.image.revision="$REVISION"

# Pas d'interpréteur de commandes dans l'image : toujours la forme en liste.
CMD ["mine-de-rien", "--config", "/app/config.toml", "lancer"]
