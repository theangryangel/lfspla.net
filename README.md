# lfspla.net

lfspla.net is a community archive for [Live for Speed](https://www.lfs.net/)
hotlaps. Players upload single-player replay (SPR) files. The site checks each
lap with LFS HLVC, then shows it in charts and rankings for its era.

This repository contains the site and the tools that run it. It is a
Linux-only Rust service with a Svelte frontend and PostgreSQL database. The web
service, replay validator, and maintenance jobs run as separate processes.

## Documentation

- [Docs](docs/)
- [Contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)
- [Security](SECURITY.md)

## Support

Use the [issue tracker](https://github.com/theangryangel/lfspla.net/issues)
for technical bugs and changes to the code. For a bug, include what you did,
what you expected, what happened, and relevant logs with secrets removed.

We know most players will not want to sign up to GitHub to discuss rankings.
Use the [LFS Planet Forum](https://www.lfs.net/forum/565-LFS-Planet-Forum) for
ranking rules, era changes, and other changes to the site. These affect the
community, so they belong in the forum rather than a technical issue.

## License

Licensed under the [MIT License](LICENSE). Live for Speed artwork and track
outlines are not covered; see [NOTICE](NOTICE.md).
