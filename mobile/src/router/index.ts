import { createRouter, createWebHashHistory } from 'vue-router'
import { updateRouteTransition } from '@/core/navigation'
const AssetsView = () => import('@/views/AssetsView.vue')
const AccountBindingsView = () => import('@/views/AccountBindingsView.vue')
const DepositAssetView = () => import('@/views/DepositAssetView.vue')
const DepositDetailView = () => import('@/views/DepositDetailView.vue')
const DepositNetworkView = () => import('@/views/DepositNetworkView.vue')
const ForgotPasswordView = () => import('@/views/ForgotPasswordView.vue')
const HomeView = () => import('@/views/HomeView.vue')
const LoginView = () => import('@/views/LoginView.vue')
const LoginTwoFactorView = () => import('@/views/LoginTwoFactorView.vue')
const LanguageView = () => import('@/views/LanguageView.vue')
const LoanView = () => import('@/views/LoanView.vue')
const MarketDetailView = () => import('@/views/MarketDetailView.vue')
const MarketsView = () => import('@/views/MarketsView.vue')
const NewsDetailView = () => import('@/views/NewsDetailView.vue')
const NewsView = () => import('@/views/NewsView.vue')
const NewCoinsView = () => import('@/views/NewCoinsView.vue')
const NewCoinDetailView = () => import('@/views/NewCoinDetailView.vue')
const NewCoinRecordsView = () => import('@/views/NewCoinRecordsView.vue')
const KycView = () => import('@/views/KycView.vue')
const OrdersView = () => import('@/views/OrdersView.vue')
const ProfileView = () => import('@/views/ProfileView.vue')
const ProductHubView = () => import('@/views/ProductHubView.vue')
const PredictionView = () => import('@/views/PredictionView.vue')
const QuickRechargeView = () => import('@/views/QuickRechargeView.vue')
const RegisterView = () => import('@/views/RegisterView.vue')
const ReferralsView = () => import('@/views/ReferralsView.vue')
const SecurityView = () => import('@/views/SecurityView.vue')
const SecondsView = () => import('@/views/SecondsView.vue')
const SwapView = () => import('@/views/SwapView.vue')
const EarnView = () => import('@/views/EarnView.vue')
const TradeView = () => import('@/views/TradeView.vue')
const WalletLedgerView = () => import('@/views/WalletLedgerView.vue')
const WithdrawAssetView = () => import('@/views/WithdrawAssetView.vue')
const WithdrawView = () => import('@/views/WithdrawView.vue')

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'home', component: HomeView, meta: { depth: 0 } },
    { path: '/markets', name: 'markets', component: MarketsView, meta: { depth: 0 } },
    { path: '/markets/:symbol', name: 'market-detail', component: MarketDetailView, props: true, meta: { showBottomNav: false, depth: 1, backFallback: '/markets' } },
    { path: '/news', name: 'news', component: NewsView, meta: { showBottomNav: false, depth: 1, backFallback: '/' } },
    { path: '/news/:id', name: 'news-detail', component: NewsDetailView, props: true, meta: { showBottomNav: false, depth: 2, backFallback: '/news' } },
    { path: '/trade/:symbol?', name: 'trade', component: TradeView, meta: { depth: 0 } },
    { path: '/swap', name: 'swap', component: SwapView, meta: { showBottomNav: false, depth: 1, backFallback: '/trade/BTC_USDT' } },
    { path: '/products', name: 'products', component: ProductHubView, meta: { showBottomNav: false, depth: 1, backFallback: '/' } },
    { path: '/products/earn', name: 'earn', component: EarnView, meta: { showBottomNav: false, depth: 2, backFallback: '/products' } },
    { path: '/products/loan', name: 'loan', component: LoanView, meta: { showBottomNav: false, depth: 2, backFallback: '/products' } },
    { path: '/products/new-coins', name: 'new-coins', component: NewCoinsView, meta: { showBottomNav: false, depth: 2, backFallback: '/products' } },
    { path: '/products/new-coins/records', name: 'new-coin-records', component: NewCoinRecordsView, meta: { showBottomNav: false, depth: 3, backFallback: '/products/new-coins' } },
    { path: '/products/new-coins/:symbol', name: 'new-coin-detail', component: NewCoinDetailView, props: true, meta: { showBottomNav: false, depth: 3, backFallback: '/products/new-coins' } },
    { path: '/products/prediction', name: 'prediction', component: PredictionView, meta: { showBottomNav: false, depth: 2, backFallback: '/products' } },
    { path: '/products/seconds', name: 'seconds', component: SecondsView, meta: { showBottomNav: false, depth: 2, backFallback: '/products' } },
    { path: '/orders', name: 'orders', component: OrdersView, meta: { showBottomNav: false, depth: 1, backFallback: '/trade/BTC_USDT' } },
    { path: '/profile', name: 'profile', component: ProfileView, meta: { depth: 0 } },
    { path: '/profile/language', name: 'language', component: LanguageView, meta: { showBottomNav: false, depth: 1, backFallback: '/profile' } },
    { path: '/profile/kyc', name: 'kyc', component: KycView, meta: { showBottomNav: false, depth: 1, backFallback: '/profile' } },
    { path: '/profile/security', name: 'security', component: SecurityView, meta: { showBottomNav: false, depth: 1, backFallback: '/profile' } },
    { path: '/profile/bindings', name: 'account-bindings', component: AccountBindingsView, meta: { showBottomNav: false, depth: 1, backFallback: '/profile' } },
    { path: '/profile/referrals', name: 'referrals', component: ReferralsView, meta: { showBottomNav: false, depth: 1, backFallback: '/profile' } },
    { path: '/assets', name: 'assets', component: AssetsView, meta: { depth: 0 } },
    { path: '/assets/deposit', name: 'deposit-asset', component: DepositAssetView, meta: { showBottomNav: false, depth: 1, backFallback: '/assets' } },
    { path: '/assets/deposit/:asset/networks', name: 'deposit-network', component: DepositNetworkView, props: true, meta: { showBottomNav: false, depth: 2, backFallback: '/assets/deposit' } },
    { path: '/assets/deposit/:asset/:network', name: 'deposit-detail', component: DepositDetailView, props: true, meta: { showBottomNav: false, depth: 3, backFallback: '/assets/deposit' } },
    { path: '/assets/withdraw', name: 'withdraw-asset', component: WithdrawAssetView, meta: { showBottomNav: false, depth: 1, backFallback: '/assets' } },
    { path: '/assets/withdraw/:asset', name: 'withdraw', component: WithdrawView, props: true, meta: { showBottomNav: false, depth: 2, backFallback: '/assets/withdraw' } },
    { path: '/assets/ledger', name: 'wallet-ledger', component: WalletLedgerView, meta: { showBottomNav: false, depth: 1, backFallback: '/assets' } },
    { path: '/assets/quick-recharge', name: 'quick-recharge', component: QuickRechargeView, meta: { showBottomNav: false, depth: 1, backFallback: '/assets' } },
    { path: '/login', name: 'login', component: LoginView, meta: { showBottomNav: false, depth: 1, backFallback: '/' } },
    { path: '/login/two-factor', name: 'login-two-factor', component: LoginTwoFactorView, meta: { showBottomNav: false, depth: 2, backFallback: '/login' } },
    { path: '/register', name: 'register', component: RegisterView, meta: { showBottomNav: false, depth: 2, backFallback: '/login' } },
    { path: '/forgot-password', name: 'forgot-password', component: ForgotPasswordView, meta: { showBottomNav: false, depth: 2, backFallback: '/login' } },
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
  scrollBehavior: (_to, _from, savedPosition) => savedPosition || ({ top: 0, left: 0 }),
})

router.beforeEach((to, from) => {
  updateRouteTransition(to.meta.depth, from.meta.depth)
})

export default router
