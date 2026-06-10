import { webStructureAnalyzer } from './build/tools/web-structure-analyzer.js';

async function main() {
  console.log("Starting analysis...");
  try {
    const result = await webStructureAnalyzer.run({
      url: "https://www.bitget.com/zh-CN/spot/BGBUSDT?type=spot"
    });

    if (result.content && result.content[0] && result.content[0].text) {
      console.log(result.content[0].text);
    } else {
      console.log("No content returned", result);
    }
  } catch (error) {
    console.error("Analysis failed:", error);
  }
}

main();
