FROM python:3.12-slim
COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv
WORKDIR /app
COPY pyproject.toml uv.lock* README.md ./
COPY src ./src
RUN uv sync --frozen --no-dev --no-cache || uv sync --no-dev --no-cache
CMD ["uv", "run", "--no-sync", "mine-de-rien", "--config", "/app/config.toml"]
