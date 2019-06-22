fn main() {
    use scraper::{Html, Selector};

    let html = r#"
        <layer id=3 name="landfill" class="mountain" />
    "#;

    let fragment = Html::parse_fragment(html);
    let selector = Selector::parse("layer[name=landfill].mountain").unwrap();

    for element in fragment.select(&selector) {
        dbg!(element.value().id());
    }

    // let html = r#"
    //     <ul>
    //         <li>Foo</li>
    //         <li>Bar</li>
    //         <li>Baz</li>
    //     </ul>
    // "#;

    // let fragment = Html::parse_fragment(html);
    // let selector = Selector::parse("li").unwrap();

    // for element in fragment.select(&selector) {
    //     dbg!(element);
    // }
}