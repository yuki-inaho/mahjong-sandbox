//! JavaScriptとの結合だけを担当します。規則やJSON処理はAPIクレートへ委譲します。

use wasm_bindgen::prelude::wasm_bindgen;

/// `request` の入力・出力はともに版付きのJSON文字列です。
#[wasm_bindgen(js_name = Engine)]
pub struct Engine {
    inner: suzume_api::Engine,
}

#[wasm_bindgen]
impl Engine {
    /// 対局開始要求を受け付ける前の状態を作ります。
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: suzume_api::Engine::new(),
        }
    }

    /// 不正な要求もAPIが定めるエラー応答へ変換します。
    pub fn request(&mut self, input: &str) -> String {
        self.inner.request(input)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
