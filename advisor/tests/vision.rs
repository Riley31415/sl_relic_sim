//! Reading the Hero's Legacy grid.  The templates are learned from the one
//! screenshot there is, so the honest checks read numbers whose own glyphs
//! were left out, and the screenshot rescaled, moved and recompressed.

use image::imageops::{self, FilterType};
use image::{Rgb, RgbImage};
use relic_advisor::vision::{self, FIXTURE_TRUTH, Screen, Spot};

/// Every number on `img` that reads differently from the fixture's truth.
fn misreads(img: &RgbImage, glyphs: &[relic_advisor::ocr::Glyph]) -> Vec<String> {
    let screen = Screen::read(img, glyphs).expect("the grid is found");
    Spot::all()
        .into_iter()
        .filter_map(|spot| {
            let want = FIXTURE_TRUTH.value(spot);
            let got = screen.value(spot);
            (got != Some(want)).then(|| format!("{}: read {got:?}, is {want}", spot.name()))
        })
        .collect()
}

#[test]
fn the_fixture_reads_back() {
    assert_eq!(misreads(&vision::fixture(), vision::templates()), Vec::<String>::new());
}

#[test]
fn the_grid_is_where_it_was_measured() {
    let frame = vision::find_frame(&vision::fixture()).unwrap();
    assert!((frame.x0 - 96.0).abs() < 0.5 && (frame.y0 - 214.0).abs() < 0.5, "{frame:?}");
    assert!((frame.s - 1.0).abs() < 0.005, "{frame:?}");
}

#[test]
fn each_number_reads_without_its_own_glyphs() {
    let img = vision::fixture();
    let mut wrong = Vec::new();
    for spot in Spot::all() {
        let glyphs = vision::learn(&img, &FIXTURE_TRUTH, |s| s == spot).unwrap();
        let frame = vision::find_frame(&img).unwrap();
        let read = vision::read_spot(&img, &frame, spot, &glyphs);
        if read.value != Some(FIXTURE_TRUTH.value(spot)) {
            wrong.push(format!("{}: read {:?}", spot.name(), read.reading.map(|r| r.text)));
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
}

#[test]
fn each_relic_reads_without_its_own_cell() {
    let img = vision::fixture();
    let frame = vision::find_frame(&img).unwrap();
    let mut wrong = Vec::new();
    for i in 0..vision::RELICS {
        let cell = [Spot::Glory(i), Spot::Despair(i), Spot::Stock(i)];
        let glyphs = vision::learn(&img, &FIXTURE_TRUTH, |s| cell.contains(&s)).unwrap();
        for spot in cell {
            let read = vision::read_spot(&img, &frame, spot, &glyphs);
            if read.value != Some(FIXTURE_TRUTH.value(spot)) {
                wrong.push(format!("{}: read {:?}", spot.name(), read.reading.map(|r| r.text)));
            }
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
}

fn scaled(img: &RgbImage, factor: f64, filter: FilterType) -> RgbImage {
    let w = (f64::from(img.width()) * factor).round() as u32;
    let h = (f64::from(img.height()) * factor).round() as u32;
    imageops::resize(img, w, h, filter)
}

#[test]
fn it_reads_at_other_sizes() {
    let img = vision::fixture();
    for factor in [0.9, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0] {
        for filter in [FilterType::Triangle, FilterType::CatmullRom, FilterType::Lanczos3] {
            let wrong = misreads(&scaled(&img, factor, filter), vision::templates());
            assert!(wrong.is_empty(), "at {factor}x ({filter:?}): {wrong:?}");
        }
    }
}

#[test]
fn it_finds_the_game_inside_a_bigger_screenshot() {
    // the cloud phone in a browser tab: dark page around it, game off-centre
    let game = scaled(&vision::fixture(), 1.3, FilterType::Triangle);
    let mut tab = RgbImage::from_pixel(1916, 1080, Rgb([24, 26, 30]));
    imageops::overlay(&mut tab, &game, 733, 41);
    let frame = vision::find_frame(&tab).unwrap();
    assert!((frame.s - 1.3).abs() < 0.01, "{frame:?}");
    assert!((frame.x0 - (733.0 + 96.5 * 1.3)).abs() < 2.0, "{frame:?}");
    assert_eq!(misreads(&tab, vision::templates()), Vec::<String>::new());
}

#[test]
fn it_reads_through_jpeg_compression() {
    let img = scaled(&vision::fixture(), 1.5, FilterType::Triangle);
    for quality in [95, 85, 75] {
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality).encode_image(&img).unwrap();
        let jpeg = image::load_from_memory(&bytes).unwrap().to_rgb8();
        let wrong = misreads(&jpeg, vision::templates());
        assert!(wrong.is_empty(), "at quality {quality}: {wrong:?}");
    }
}

#[test]
fn a_different_screen_is_refused() {
    let blank = RgbImage::from_pixel(450, 816, Rgb([10, 10, 10]));
    let err = vision::find_frame(&blank).unwrap_err();
    assert!(err.contains("not the Hero's Legacy grid"), "{err}");
}

/// Read every `which` spot with templates learned from everything else.
fn holdout(which: impl Fn(Spot) -> bool) -> Vec<String> {
    let img = vision::fixture();
    let frame = vision::find_frame(&img).unwrap();
    let glyphs = vision::learn(&img, &FIXTURE_TRUTH, &which).unwrap();
    Spot::all()
        .into_iter()
        .filter(|&s| which(s))
        .filter_map(|spot| {
            let read = vision::read_spot(&img, &frame, spot, &glyphs);
            (read.value != Some(FIXTURE_TRUTH.value(spot)))
                .then(|| format!("{}: read {:?}", spot.name(), read.reading.map(|r| r.text)))
        })
        .collect()
}

#[test]
fn despair_reads_without_any_red_glyphs() {
    // a despair count never seen in red (x 0, x 4) has only other colours' glyphs
    let red = |s: Spot| matches!(s, Spot::Despair(_) | Spot::TotalDespair | Spot::NeedDespair);
    assert_eq!(holdout(red), Vec::<String>::new());
}

#[test]
fn glory_reads_without_any_gold_glyphs() {
    let gold = |s: Spot| matches!(s, Spot::Glory(_) | Spot::TotalGlory | Spot::NeedGlory);
    assert_eq!(holdout(gold), Vec::<String>::new());
}

// (no such check for the white stock counts: 0, 5 and 6 are drawn only in
// white on the fixture, and are what lets the coloured counts read them)
