FROM python:3.12-slim
COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv
WORKDIR /app
# Journaux visibles tout de suite dans `docker compose logs agent`.
ENV PYTHONUNBUFFERED=1
COPY pyproject.toml uv.lock* LICENSE README.md ./
COPY src ./src
RUN uv sync --frozen --no-dev --no-cache || uv sync --no-dev --no-cache
CMD ["uv", "run", "--no-sync", "mine-de-rien", "--config", "/app/config.toml", "lancer"]
