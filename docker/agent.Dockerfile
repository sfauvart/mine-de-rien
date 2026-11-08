# Image de l'agent, datapack compris. Les versions de base sont figées : deux constructions
# du même commit donnent la même image.
FROM ghcr.io/astral-sh/uv:0.13.7 AS uv

FROM python:3.12.16-slim-trixie
COPY --from=uv /uv /usr/local/bin/uv
WORKDIR /app
# Journaux visibles tout de suite dans `docker compose logs agent`.
ENV PYTHONUNBUFFERED=1
COPY pyproject.toml uv.lock LICENSE README.md ./
COPY src ./src
# Projet installé en mode éditable : `mine-de-rien datapack` trouve le datapack dans /app.
RUN uv sync --frozen --no-dev --no-cache
COPY datapacks ./datapacks
# `mine-de-rien` se lance directement ; `uv run --no-sync mine-de-rien` marche toujours.
ENV PATH=/app/.venv/bin:$PATH

ARG VERSION=dev
ARG REVISION=inconnue
LABEL org.opencontainers.image.title="mine-de-rien" \
      org.opencontainers.image.description="Agent Mine de rien et son datapack : les devoirs de CM2 mis en jeu dans Minecraft." \
      org.opencontainers.image.source="https://github.com/sfauvart/mine-de-rien" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="$VERSION" \
      org.opencontainers.image.revision="$REVISION"

CMD ["mine-de-rien", "--config", "/app/config.toml", "lancer"]
