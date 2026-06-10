import axios from 'axios';
import * as cheerio from 'cheerio';

async function analyze() {
    const url = "https://www.bitget.com/zh-CN/spot/BGBUSDT?type=spot";
    console.log(`Analyzing ${url}...`);

    try {
        const response = await axios.get(url, {
            headers: {
                'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
                'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7',
                'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.8',
                'Accept-Encoding': 'gzip, deflate, br',
                'Cache-Control': 'max-age=0',
                'Connection': 'keep-alive',
                'Upgrade-Insecure-Requests': '1',
                'Sec-Fetch-Dest': 'document',
                'Sec-Fetch-Mode': 'navigate',
                'Sec-Fetch-Site': 'none',
                'Sec-Fetch-User': '?1',
                'Pragma': 'no-cache'
            },
            timeout: 30000
        });

        const html = response.data;
        const $ = cheerio.load(html);
        const title = $('title').text().trim();
        console.log("Title:", title);

        // Basic Layout Analysis
        const mainClasses = [];
        $('div[class*="layout"], div[class*="container"], div[class*="wrapper"]').each((i, el) => {
            mainClasses.push($(el).attr('class'));
        });

        console.log("Potential Layout Containers:", mainClasses.slice(0, 10));

    } catch (error) {
        console.error("Analysis failed:", error.message);
        if (error.response) {
            console.error("Status:", error.response.status);
            console.error("Headers:", error.response.headers);
        }
    }
}

analyze();
