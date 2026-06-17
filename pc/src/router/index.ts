import { createRouter, createWebHistory } from 'vue-router'
import MainLayout from '@/components/layout/MainLayout.vue'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/login',
      name: 'Login',
      component: () => import('@/views/auth/Login.vue')
    },
    {
      path: '/register',
      name: 'Register',
      component: () => import('@/views/auth/Register.vue')
    },
    {
      path: '/forgot-password',
      name: 'ForgotPassword',
      component: () => import('@/views/auth/ForgotPassword.vue')
    },
    {
      path: '/',
      component: MainLayout,
      children: [
        {
          path: '/',
          name: 'Home',
          component: () => import('@/views/Home.vue')
        },
        {
          path: 'news',
          name: 'News',
          component: () => import('@/views/News.vue')
        },
        {
          path: 'news/detail/:id',
          name: 'NewsDetail',
          component: () => import('@/views/News.vue')
        },
        {
          path: 'market',
          name: 'Market',
          component: () => import('@/views/Market.vue')
        },
        {
          path: 'contract/:symbol?',
          name: 'Contract',
          component: () => import('@/views/Contract.vue')
        },
        {
          path: 'otc',
          name: 'OTC',
          component: () => import('@/views/OTC.vue')
        },
        {
          path: 'swap',
          name: 'Swap',
          component: () => import('@/views/Swap.vue')
        },
          {
              path: 'spot/:symbol?',
              name: 'Trade',
              component: () => import('@/views/Trade.vue')
          },
        {
          path: 'second/:symbol?',
          name: 'SecondOptions',
          component: () => import('@/views/SecondOptions.vue')
        },

        {
          path: 'launchpad',
          name: 'Launchpad',
          component: () => import('@/views/Launchpad.vue')
        },
        {
          path: 'launchpad/trade/:symbol?',
          name: 'LaunchpadTrade',
          component: () => import('@/views/LaunchpadTrade.vue')
        },
        {
          path: 'finance',
          name: 'Finance',
          component: () => import('@/views/Finance.vue')
        },
        {
            path: 'loan',
            name: 'Loan',
            component: () => import('@/views/Loan.vue')
        },
        {
          path: 'prediction',
          name: 'Prediction',
          component: () => import('@/views/Prediction.vue')
        },
        {
          path: 'user',
          component: () => import('@/views/User/UserLayout.vue'),
          redirect: '/user/security',
          children: [
            {
              path: 'kyc',
              name: 'KYC',
              component: () => import('@/views/User/KYC.vue')
            },
            {
              path: 'assets',
              name: 'Assets',
              component: () => import('@/views/User/Assets.vue')
            },
            {
              path: 'security',
              name: 'Security',
              component: () => import('@/views/User/Security.vue')
            },
            {
              path: 'invite',
              name: 'Invite',
              component: () => import('@/views/User/Invite.vue')
            },
            {
              path: 'transaction',
              name: 'Transaction',
              component: () => import('@/views/User/Transaction.vue')
            },
            {
              path: 'recharge',
              name: 'Recharge',
              component: () => import('@/views/User/Recharge.vue')
            },
            {
              path: 'withdraw',
              name: 'Withdraw',
              component: () => import('@/views/User/Withdraw.vue')
            },
            {
              path: 'loan-orders',
              name: 'LoanOrders',
              component: () => import('@/views/User/LoanOrders.vue')
            },
            {
              path: 'prediction-orders',
              name: 'PredictionOrders',
              component: () => import('@/views/User/PredictionOrders.vue')
            },
            {
              path: 'finance-orders',
              name: 'FinanceOrders',
              component: () => import('@/views/User/FinanceOrders.vue')
            }
          ]
        }
      ]
    }
  ]
})

export default router
