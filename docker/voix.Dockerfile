# Voix de synthèse locale : Piper et son serveur HTTP, pour lire les dictées.
#
# Piper est sous licence GPL : il vit dans cette image à part, jamais dans le binaire de
# l'agent, qui lui parle par HTTP. Les versions sont figées.
#
# Les voix ne sont pas dans l'image : elles se téléchargent une fois dans data/voix
# (docs/installation.md). Le serveur, lui, tourne sur un réseau sans sortie.
FROM python:3.12.16-slim-trixie
RUN pip install --no-cache-dir "piper-tts[http]==1.8.0"
WORKDIR /voix
EXPOSE 5000
ENTRYPOINT ["python", "-m"]
CMD ["piper.http_server", "--host", "0.0.0.0", "--port", "5000", "--data-dir", "/voix", "--model", "fr_FR-siwis-medium"]
