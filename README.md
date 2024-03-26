# DIY Link Shortener in Rust

**Author:** Oliver Jumpertz  
**YouTube Link:** [Watch the video](https://www.youtube.com/watch?v=9KkTd4eDUMY)

Are you familiar with tools like Bitly, t.ly, TinyURL, and others? They all serve as link (or URL) shorteners. But did you know that their core logic isn't that difficult to implement yourself?

What if you could create your very own link shortener, in Rust, as a perfect learning project and one that actually provides value to yourself?

This is what this video is about. We implement our very own link shortener in Rust using the following technologies:

- **Rust**: Our beloved programming language of choice
- **Axum**: As the web server
- **sqlx**: As our persistence library without the need for a fully-fledged ORM
- **Prometheus**: For metrics
- **Open Telemetry**: For tracing and logging
- And a few more

The best part? Once you're done, you can easily containerize the application and deploy it anywhere you like. Just a bit of further setup (depending on where you deploy it), and you can shorten your own links and distribute them wherever you prefer!
